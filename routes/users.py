"""User profile routes — /api/users"""
from datetime import datetime
from flask.views import MethodView
from flask_smorest import Blueprint as SmorestBlueprint, abort
from flask import request, current_app, session
from werkzeug.utils import secure_filename
import uuid, os

from schemas import (
    UserProfile,
    UserSettingsUpdate,
    ProfilePicResponse,
    PushSubscriptionRequest,
    VapidPublicKeyResponse,
)
from routes import require_auth, require_admin
from services.users import get_profile, serialize_profile


users_blp = SmorestBlueprint("users", __name__, url_prefix="/api/users",
                              description="User profiles and account management")


# ── /me ───────────────────────────────────────────────────────────────────

@users_blp.route("/me")
class CurrentUserResource(MethodView):
    @users_blp.response(200, UserProfile)
    def get(self):
        """Get the authenticated user's profile"""
        require_auth()
        from app import DOMAIN
        return serialize_profile(request.user, DOMAIN)

    @users_blp.arguments(UserSettingsUpdate)
    @users_blp.response(200, UserProfile)
    def patch(self, data):
        """Update the authenticated user's settings"""
        require_auth()
        from app import DOMAIN
        user = request.user
        db = current_app.extensions['sqlalchemy']

        if "display_name" in data:
            user.display_name = data["display_name"]

        db.session.commit()
        return serialize_profile(user, DOMAIN)


# ── /me/picture ───────────────────────────────────────────────────────────

@users_blp.route("/push/vapid_public_key")
class VapidPublicKeyResource(MethodView):
    @users_blp.response(200, VapidPublicKeyResponse)
    def get(self):
        """Get VAPID public key for browser push subscription."""
        return {"publicKey": current_app.config.get("VAPID_PUBLIC_KEY", "")}


@users_blp.route("/me/push/subscribe")
class PushSubscriptionResource(MethodView):
    @users_blp.arguments(PushSubscriptionRequest)
    @users_blp.response(200)
    def post(self, data):
        """Save or update a browser push subscription for the authenticated user."""
        require_auth()
        from app import PushSubscription

        db = current_app.extensions['sqlalchemy']
        endpoint = data.get('endpoint')
        keys = data.get('keys') or {}
        if not endpoint or not keys.get('p256dh') or not keys.get('auth'):
            abort(400, message='Invalid push subscription payload')

        subs = db.session.query(PushSubscription).filter_by(endpoint=endpoint).all()
        if subs:
            sub = subs[0]
            sub.user_id = request.user.id
            sub.p256dh = keys['p256dh']
            sub.auth = keys['auth']
        else:
            sub = PushSubscription(
                user_id=request.user.id,
                endpoint=endpoint,
                p256dh=keys['p256dh'],
                auth=keys['auth'],
            )
            db.session.add(sub)
        db.session.commit()
        return {"success": True}

    @users_blp.response(200)
    def delete(self):
        """Remove a browser push subscription for the authenticated user."""
        require_auth()
        from app import PushSubscription

        db = current_app.extensions['sqlalchemy']
        endpoint = request.args.get('endpoint')
        query = db.session.query(PushSubscription).filter_by(user_id=request.user.id)
        if endpoint:
            query = query.filter_by(endpoint=endpoint)
        deleted = query.delete()
        db.session.commit()
        return {"deleted": deleted}


@users_blp.route("/me/activity")
class UserActivityResource(MethodView):
    @users_blp.response(200)
    def post(self):
        """Record a browser/activity ping for the authenticated user."""
        require_auth()
        from app import UserSession

        db = current_app.extensions['sqlalchemy']
        token = session.get('session_token')
        if token:
            user_session = db.session.query(UserSession).filter_by(token=token).first()
            if user_session and user_session.expires_at > datetime.utcnow():
                try:
                    user_session.last_active = datetime.utcnow()
                    if request.user.Status_type not in (2, 3):
                        request.user.Status_type = 0
                    db.session.commit()
                except Exception:
                    db.session.rollback()
        return {"success": True}


@users_blp.route("/me/picture")
class ProfilePictureResource(MethodView):
    @users_blp.response(200, ProfilePicResponse)
    def post(self):
        """Upload a new profile picture"""
        require_auth()
        from app import DOMAIN
        db = current_app.extensions['sqlalchemy']

        if "file" not in request.files:
            abort(400, message="No file provided")
        file = request.files["file"]
        if not file or file.filename == "":
            abort(400, message="Invalid file")

        allowed = {"image/png", "image/jpeg", "image/jpg", "image/gif", "image/webp", "image/svg+xml"}
        if file.mimetype not in allowed:
            abort(400, message="Unsupported file type")

        filename = secure_filename(f"{uuid.uuid4()}_{file.filename}")
        save_path = os.path.join(current_app.config["UPLOAD_FOLDER"], filename)
        file.save(save_path)

        direct_url = f"http://{DOMAIN}/uploads/{filename}"
        pic_url = f"http://{DOMAIN}/media/proxy?url={direct_url}"

        request.user.profile_pic = pic_url
        db.session.commit()
        return {"profile_pic": pic_url}

    @users_blp.response(200, ProfilePicResponse)
    def delete(self):
        """Remove profile picture"""
        require_auth()
        db = current_app.extensions['sqlalchemy']
        request.user.profile_pic = None
        db.session.commit()
        return {"profile_pic": None}


# ── /<username@domain> ────────────────────────────────────────────────────

@users_blp.route("/<string:full_id>")
class UserProfileResource(MethodView):
    @users_blp.response(200, UserProfile)
    def get(self, full_id):
        """Get a user's profile by username@domain (federation-aware, webhook-aware)"""
        if "@" not in full_id:
            abort(400, message="Invalid format. Use username@domain")

        from app import DOMAIN, Webhook
        db = current_app.extensions['sqlalchemy']
        username, domain = full_id.rsplit("@", 1)

        # Check if it's a webhook sender on the local domain
        if domain == DOMAIN and username.startswith("webhook-"):
            webhook_id = username[8:]  # Remove "webhook-" prefix
            webhook = db.session.query(Webhook).filter_by(id=webhook_id).first()
            if webhook:
                return {
                    "username": full_id,
                    "display_name": webhook.name,
                    "profile_pic": webhook.profile_pic,
                    "status": "offline",
                    "is_admin": False,
                }

        try:
            profile = get_profile(full_id)
        except ConnectionError:
            abort(502, message="Failed to reach remote server")

        if not profile:
            abort(404, message="User not found")
        return profile

    @users_blp.response(200)
    def delete(self, full_id):
        """Deactivate a user (admin only)"""
        require_admin()
        from app import User, DOMAIN

        if "@" in full_id:
            username, domain = full_id.rsplit("@", 1)
            if domain != DOMAIN:
                abort(400, message="Cannot delete users on remote servers")
        else:
            username = full_id

        db = current_app.extensions['sqlalchemy']
        user = db.session.query(User).filter_by(username=username).first()
        if not user:
            abort(404, message="User not found")
        if user.is_admin:
            abort(400, message="Cannot delete admin user")
        user.is_deleted = True
        user.sessions = []
        db.session.commit()
        return {"message": f"User {user.username} has been deactivated."}


# ── / (admin list) ────────────────────────────────────────────────────────

@users_blp.route("/")
class UserListResource(MethodView):
    @users_blp.response(200, UserProfile(many=True))
    def get(self):
        """List all users (admin only)"""
        require_admin()
        from app import User, DOMAIN
        db = current_app.extensions['sqlalchemy']
        users = db.session.query(User).all()
        return [serialize_profile(u, DOMAIN) for u in users]
