"""User profile routes — /api/users"""
from flask.views import MethodView
from flask_smorest import Blueprint as SmorestBlueprint, abort
from flask import request, current_app
from werkzeug.utils import secure_filename
import uuid, os

from schemas import UserProfile, UserSettingsUpdate, ProfilePicResponse, BatchProfileRequest
from routes import require_auth, require_admin, get_user_status

users_blp = SmorestBlueprint("users", __name__, url_prefix="/api/users",
                              description="User profiles and account management")


def _serialize_profile(user, domain):
    """Convert a User model to a public profile dict — no internal IDs exposed."""
    return {
        "username": f"{user.username}@{domain}",
        "display_name": user.display_name or user.username,
        "profile_pic": user.profile_pic or None,
        "status": get_user_status(user),
    }


# ── /me ───────────────────────────────────────────────────────────────────

@users_blp.route("/me")
class CurrentUserResource(MethodView):
    @users_blp.response(200, UserProfile)
    def get(self):
        """Get the authenticated user's profile"""
        require_auth()
        from app import DOMAIN
        return _serialize_profile(request.user, DOMAIN)

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
        return _serialize_profile(user, DOMAIN)


# ── /me/picture ───────────────────────────────────────────────────────────

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
        """Get a user's profile by username@domain (federation-aware)"""
        if "@" not in full_id:
            abort(400, message="Invalid format. Use username@domain")

        username, domain = full_id.rsplit("@", 1)
        from app import User, DOMAIN

        if domain == DOMAIN:
            db = current_app.extensions['sqlalchemy']
            user = db.session.query(User).filter_by(
                username=username, is_deleted=False
            ).first()
            if not user:
                abort(404, message="User not found")
            return _serialize_profile(user, DOMAIN)

        # Remote user — fetch via federation
        import requests as http_requests
        from json_discovery import get_endpoint
        try:
            base = get_endpoint(domain, "userserver", "users")
            if not base:
                base = f"http://{domain}/api/users"
            resp = http_requests.get(f"{base}/{full_id}", timeout=3)
            if resp.status_code == 200:
                return resp.json()
            abort(404, message="User not found on remote server")
        except http_requests.RequestException:
            abort(502, message="Failed to reach remote server")

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


# ── /batch ────────────────────────────────────────────────────────────────

@users_blp.route("/batch")
class BatchProfilesResource(MethodView):
    @users_blp.arguments(BatchProfileRequest)
    @users_blp.response(200)
    def post(self, data):
        """Fetch profiles for multiple users at once"""
        require_auth()
        import requests as http_requests
        from app import User, DOMAIN
        from json_discovery import get_endpoint
        db = current_app.extensions['sqlalchemy']
        senders = data.get("senders", [])
        profiles = {}

        for sender in senders:
            if "@" not in sender:
                continue
            username, domain = sender.rsplit("@", 1)
            if domain == DOMAIN:
                user = db.session.query(User).filter_by(username=username).first()
                profiles[sender] = _serialize_profile(user, DOMAIN) if user else None
            else:
                try:
                    base = get_endpoint(domain, "userserver", "users")
                    if not base:
                        base = f"http://{domain}/api/users"
                    resp = http_requests.get(f"{base}/{sender}", timeout=1)
                    if resp.status_code == 200:
                        profiles[sender] = resp.json()
                    else:
                        profiles[sender] = None
                except Exception:
                    profiles[sender] = None

        return profiles


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
        return [_serialize_profile(u, DOMAIN) for u in users]
