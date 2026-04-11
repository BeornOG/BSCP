"""Standardized REST API v1 for BSCP — uses flask-smorest for OpenAPI docs."""
from flask.views import MethodView
from flask_smorest import Blueprint as SmorestBlueprint, abort
from flask import request, session, current_app
from datetime import datetime, timedelta
from werkzeug.security import generate_password_hash, check_password_hash
from werkzeug.utils import secure_filename
import pyotp, secrets, uuid, os

from schemas import (
    UserObject, MessageObject, ChatObject, InviteObject,
    LoginRequest, LoginResponse, TwoFactorRequest,
    SetupRequest, SetupStatusResponse, RegisterRequest,
    AuthSuccessResponse, AuthErrorResponse,
    SendMessageRequest, SendMessageResponse, MessagesQueryArgs,
    UserSettingsUpdate, ProfilePicResponse,
    UploadResponse, BatchProfileRequest,
)


# ═══════════════════════════════════════════════════════════════════════════
# AUTH
# ═══════════════════════════════════════════════════════════════════════════

auth_blp = SmorestBlueprint("auth", __name__, url_prefix="/api/auth",
                             description="Authentication & account management")


@auth_blp.route("/setup")
class SetupResource(MethodView):
    @auth_blp.response(200, SetupStatusResponse)
    def get(self):
        """Check if initial setup is needed"""
        from app import User
        db = current_app.extensions['sqlalchemy']
        return {"needs_setup": db.session.query(User).count() == 0}

    @auth_blp.arguments(SetupRequest)
    @auth_blp.response(201, AuthSuccessResponse)
    @auth_blp.alt_response(400, schema=AuthErrorResponse)
    def post(self, data):
        """Create initial admin account"""
        from app import User
        db = current_app.extensions['sqlalchemy']

        if db.session.query(User).count() > 0:
            abort(400, message="Setup already complete")

        errors = []
        username = data.get("username", "").strip()
        password = data.get("password", "")
        password_confirm = data.get("password_confirm", "")
        email = (data.get("email") or "").strip() or None

        if not username:
            errors.append("Username is required")
        elif len(username) < 3:
            errors.append("Username must be at least 3 characters")
        if not password:
            errors.append("Password is required")
        elif len(password) < 6:
            errors.append("Password must be at least 6 characters")
        if password != password_confirm:
            errors.append("Passwords do not match")

        if errors:
            abort(400, message=", ".join(errors), errors=errors)

        user = User(
            username=username,
            password_hash=generate_password_hash(password),
            email=email,
            is_admin=True,
            otp_secret=pyotp.random_base32(),
            is_2fa_enabled=False,
        )
        db.session.add(user)
        db.session.commit()
        return {"success": True}


@auth_blp.route("/login")
class LoginResource(MethodView):
    @auth_blp.arguments(LoginRequest)
    @auth_blp.response(200, LoginResponse)
    def post(self, data):
        """Authenticate with username and password"""
        from app import User, UserSession
        db = current_app.extensions['sqlalchemy']

        user = db.session.query(User).filter_by(username=data["user"]).first()
        if not user or not check_password_hash(user.password_hash, data["password"]):
            return {"success": False, "error": "Invalid username or password"}

        if user.is_2fa_enabled:
            session["pending_user_id"] = user.id
            return {"success": False, "requires_2fa": True}

        _create_session(db, user)
        return {"success": True}


@auth_blp.route("/2fa")
class TwoFactorResource(MethodView):
    @auth_blp.arguments(TwoFactorRequest)
    @auth_blp.response(200, AuthSuccessResponse)
    def post(self, data):
        """Verify 2FA one-time code"""
        from app import User, UserSession
        db = current_app.extensions['sqlalchemy']

        user_id = session.get("pending_user_id")
        if not user_id:
            abort(400, message="No pending 2FA session")

        user = db.session.query(User).get(user_id)
        if not user:
            abort(400, message="User not found")

        totp = pyotp.TOTP(user.otp_secret)
        if not totp.verify(data["otp"]):
            return {"success": False, "error": "Invalid code"}

        _create_session(db, user)
        return {"success": True}


@auth_blp.route("/register")
class RegisterResource(MethodView):
    @auth_blp.arguments(RegisterRequest)
    @auth_blp.response(201, AuthSuccessResponse)
    def post(self, data):
        """Register a new account with an invite code"""
        from app import User, InviteCode
        db = current_app.extensions['sqlalchemy']

        if db.session.query(User).count() == 0:
            abort(400, message="Setup required first")

        errors = []
        username = data.get("username", "").strip()
        password = data.get("password", "")
        password_confirm = data.get("password_confirm", "")
        invite_code = data.get("invite_code", "").strip()

        if not username:
            errors.append("Username is required")
        elif len(username) < 3:
            errors.append("Username must be at least 3 characters")
        elif db.session.query(User).filter_by(username=username).first():
            errors.append("Username already exists")
        if not password:
            errors.append("Password is required")
        elif len(password) < 6:
            errors.append("Password must be at least 6 characters")
        if password != password_confirm:
            errors.append("Passwords do not match")
        if not invite_code:
            errors.append("Invite code is required")

        if not errors:
            invite = db.session.query(InviteCode).filter_by(code=invite_code).first()
            if not invite:
                errors.append("Invalid invite code")
            elif invite.used_by is not None:
                errors.append("Invite code already used")
            elif invite.expires_at and invite.expires_at < datetime.utcnow():
                errors.append("Invite code has expired")

        if errors:
            abort(400, message=", ".join(errors), errors=errors)

        user = User(
            id=str(uuid.uuid4()),
            username=username,
            password_hash=generate_password_hash(password),
        )
        db.session.add(user)
        db.session.flush()

        invite.used_by = user.id
        invite.used_at = datetime.utcnow()
        db.session.commit()
        return {"success": True}


@auth_blp.route("/logout")
class LogoutResource(MethodView):
    @auth_blp.response(200, AuthSuccessResponse)
    def post(self):
        """Log out and destroy session"""
        from app import UserSession
        db = current_app.extensions['sqlalchemy']
        token = session.get("session_token")
        if token:
            us = db.session.query(UserSession).filter_by(token=token).first()
            if us:
                db.session.delete(us)
                db.session.commit()
        session.clear()
        return {"success": True}


def _create_session(db, user):
    """Helper — creates a new device session and stores it in the flask session."""
    from app import UserSession
    device_token = secrets.token_urlsafe(32)
    new_session = UserSession(
        id=str(uuid.uuid4()),
        user_id=user.id,
        token=device_token,
        device_info=request.headers.get("User-Agent", "Unknown"),
        expires_at=datetime.utcnow() + timedelta(days=30),
    )
    db.session.add(new_session)
    db.session.commit()
    session.clear()
    session["session_token"] = device_token


# ═══════════════════════════════════════════════════════════════════════════
# USERS / PROFILE
# ═══════════════════════════════════════════════════════════════════════════

users_blp = SmorestBlueprint("users", __name__, url_prefix="/api/users",
                              description="User profiles and account management")


@users_blp.route("/me")
class CurrentUserResource(MethodView):
    @users_blp.response(200, UserObject)
    def get(self):
        """Get the authenticated user's profile"""
        _require_auth()
        from app import DOMAIN
        user = request.user
        return _serialize_user(user, DOMAIN)

    @users_blp.arguments(UserSettingsUpdate)
    @users_blp.response(200, UserObject)
    def patch(self, data):
        """Update the authenticated user's settings"""
        _require_auth()
        from app import DOMAIN
        user = request.user
        db = current_app.extensions['sqlalchemy']

        if "display_name" in data:
            user.display_name = data["display_name"]

        db.session.commit()
        return _serialize_user(user, DOMAIN)


@users_blp.route("/me/picture")
class ProfilePictureResource(MethodView):
    @users_blp.response(200, ProfilePicResponse)
    def post(self):
        """Upload a new profile picture"""
        _require_auth()
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
        _require_auth()
        db = current_app.extensions['sqlalchemy']
        request.user.profile_pic = None
        db.session.commit()
        return {"profile_pic": None}


@users_blp.route("/<string:username>/profile")
class PublicUserProfile(MethodView):
    @users_blp.response(200, UserObject)
    def get(self, username):
        """Get a user's public profile by username"""
        from app import User, DOMAIN
        db = current_app.extensions['sqlalchemy']
        user = db.session.query(User).filter_by(username=username).first()
        if not user:
            abort(404, message="User not found")
        return _serialize_user(user, DOMAIN)


@users_blp.route("/batch")
class BatchProfilesResource(MethodView):
    @users_blp.arguments(BatchProfileRequest)
    @users_blp.response(200)
    def post(self, data):
        """Fetch profile pictures for multiple users at once"""
        _require_auth()
        import requests as http_requests
        from app import User, DOMAIN
        db = current_app.extensions['sqlalchemy']
        senders = data.get("senders", [])
        profiles = {}

        for sender in senders:
            if "@" not in sender:
                continue
            username, domain = sender.rsplit("@", 1)
            if domain == DOMAIN:
                user = db.session.query(User).filter_by(username=username).first()
                profiles[sender] = user.profile_pic if user else None
            else:
                try:
                    resp = http_requests.get(f"http://{domain}/api/users/{username}/profile", timeout=1)
                    if resp.status_code == 200:
                        profiles[sender] = resp.json().get("profile_pic")
                    else:
                        profiles[sender] = None
                except Exception:
                    profiles[sender] = None

        return profiles


@users_blp.route("/")
class UserListResource(MethodView):
    @users_blp.response(200, UserObject(many=True))
    def get(self):
        """List all users (admin only)"""
        _require_admin()
        from app import User, DOMAIN
        db = current_app.extensions['sqlalchemy']
        users = db.session.query(User).all()
        return [_serialize_user(u, DOMAIN) for u in users]


@users_blp.route("/<string:user_id>")
class UserDetailResource(MethodView):
    @users_blp.response(200)
    def delete(self, user_id):
        """Delete (deactivate) a user (admin only)"""
        _require_admin()
        from app import User
        db = current_app.extensions['sqlalchemy']
        user = db.session.query(User).get_or_404(user_id)
        if user.is_admin:
            abort(400, message="Cannot delete admin user")
        user.is_deleted = True
        user.sessions = []
        db.session.commit()
        return {"message": f"User {user.username} has been deactivated."}


# ═══════════════════════════════════════════════════════════════════════════
# CHATS
# ═══════════════════════════════════════════════════════════════════════════

chats_blp = SmorestBlueprint("chats", __name__, url_prefix="/api/chats",
                              description="Chat conversations")


@chats_blp.route("/")
class ChatListResource(MethodView):
    @chats_blp.response(200, ChatObject(many=True))
    def get(self):
        """List all conversations for the authenticated user"""
        _require_auth()
        from app import Message, User, DOMAIN
        db = current_app.extensions['sqlalchemy']
        me = f"{request.user.username}@{DOMAIN}"

        sent_to = set(r[0] for r in
            db.session.query(Message.receiver).filter(Message.sender == me).distinct())
        received_from = set(r[0] for r in
            db.session.query(Message.sender).filter(Message.receiver == me).distinct())

        partners = sent_to | received_from
        chats = []

        for partner in sorted(partners):
            display_name = partner.split("@")[0]
            profile_pic = None
            if "@" in partner:
                uname, domain = partner.split("@", 1)
                if domain == DOMAIN:
                    u = db.session.query(User).filter_by(username=uname).first()
                    if u:
                        if u.display_name:
                            display_name = u.display_name
                        profile_pic = u.profile_pic

            chats.append({"id": partner, "display_name": display_name, "profile_pic": profile_pic})

        return chats


# ═══════════════════════════════════════════════════════════════════════════
# MESSAGES
# ═══════════════════════════════════════════════════════════════════════════

messages_blp = SmorestBlueprint("messages", __name__, url_prefix="/api/messages",
                                 description="Send and receive messages")


@messages_blp.route("/<path:target>")
class MessagesByChat(MethodView):
    @messages_blp.arguments(MessagesQueryArgs, location="query")
    @messages_blp.response(200, MessageObject(many=True))
    def get(self, args, target):
        """Get messages for a conversation"""
        _require_auth()
        from app import Message, DOMAIN
        from json_discovery import get_endpoint
        from sqlalchemy import or_
        import requests as http_requests

        db = current_app.extensions['sqlalchemy']
        me = f"{request.user.username}@{DOMAIN}"

        # Channel server (external)
        if "#" in target:
            target_domain = target.split("#")[0]
            url = get_endpoint(target_domain, "channelserver", "channel_poll")
            if not url:
                url = f"http://{target_domain}/api/channel/poll"
            params = {"path": target, "limit": args["limit"], "since": args.get("since"), "before": args.get("before")}
            try:
                return http_requests.get(url, params=params).json()
            except Exception:
                return []

        target_full = f"{target}@{DOMAIN}" if "@" not in target else target

        sent = (Message.sender == me) & (Message.receiver == target_full)
        received = (Message.sender == target_full) & (Message.receiver == me)
        query = db.session.query(Message).filter(or_(sent, received))

        if args.get("since"):
            query = query.filter(Message.timestamp > datetime.fromtimestamp(args["since"]))
        if args.get("before"):
            query = query.filter(Message.timestamp < datetime.fromtimestamp(args["before"]))

        msgs = query.order_by(Message.timestamp.desc()).limit(args["limit"]).all()

        return [
            {
                "id": m.id,
                "sender": m.sender,
                "receiver": m.receiver,
                "text": m.text,
                "timestamp": m.timestamp.timestamp(),
                "is_read": m.is_read,
            }
            for m in reversed(msgs)
        ]


@messages_blp.route("/")
class SendMessageResource(MethodView):
    @messages_blp.arguments(SendMessageRequest)
    @messages_blp.response(201, SendMessageResponse)
    def post(self, data):
        """Send a new message"""
        _require_auth()
        from app import Message as MsgModel, DOMAIN
        from json_discovery import get_endpoint
        import requests as http_requests

        msg_uuid = str(uuid.uuid4())
        full_id = f"{DOMAIN}/{msg_uuid}"
        val_key = "key-" + msg_uuid[:8]

        receiver_raw = data["receiver"]
        receiver_normalized = f"{receiver_raw}@{DOMAIN}" if "@" not in receiver_raw else receiver_raw
        sender = f"{request.user.username}@{DOMAIN}"

        new_msg = MsgModel(
            id=full_id,
            sender=sender,
            receiver=receiver_normalized,
            text=data["text"],
            validation_key=val_key,
        )
        db = current_app.extensions['sqlalchemy']
        db.session.add(new_msg)
        db.session.commit()

        msg_payload = {
            "id": full_id,
            "sender": sender,
            "receiver": receiver_normalized,
            "text": data["text"],
            "validationKey": val_key,
        }

        # Federate
        if "#" in receiver_raw:
            target_domain = receiver_raw.split("#")[0]
            url = get_endpoint(target_domain, "channelserver", "channel_send")
            if not url:
                url = f"http://{target_domain}/api/channel/send"
            try:
                http_requests.post(url, json=msg_payload, timeout=3)
            except Exception:
                pass
        else:
            target_domain = receiver_normalized.split("@")[-1]
            url = get_endpoint(target_domain, "userserver", "federation_receive")
            if not url:
                url = f"http://{target_domain}/federation/receive"
            try:
                http_requests.post(url, json=msg_payload, timeout=5)
            except Exception:
                pass

        return {
            "message": {
                "id": full_id,
                "sender": sender,
                "receiver": receiver_normalized,
                "text": data["text"],
                "timestamp": new_msg.timestamp.timestamp(),
                "is_read": False,
            }
        }


# ═══════════════════════════════════════════════════════════════════════════
# UPLOADS
# ═══════════════════════════════════════════════════════════════════════════

uploads_blp = SmorestBlueprint("uploads", __name__, url_prefix="/api/upload",
                                description="File uploads")


@uploads_blp.route("/")
class UploadResource(MethodView):
    @uploads_blp.response(201, UploadResponse)
    def post(self):
        """Upload a file and get a markdown embed"""
        _require_auth()
        from app import DOMAIN
        if "file" not in request.files:
            abort(400, message="No file provided")
        file = request.files["file"]
        if file.filename == "":
            abort(400, message="No filename")

        filename = secure_filename(f"{uuid.uuid4()}_{file.filename}")
        file.save(os.path.join(current_app.config["UPLOAD_FOLDER"], filename))

        file_url = f"http://{DOMAIN}/uploads/{filename}"
        return {"markdown": f"![image]({file_url})", "url": file_url}


# ═══════════════════════════════════════════════════════════════════════════
# INVITES (admin)
# ═══════════════════════════════════════════════════════════════════════════

invites_blp = SmorestBlueprint("invites", __name__, url_prefix="/api/invites",
                                description="Invite code management (admin)")


@invites_blp.route("/")
class InviteListResource(MethodView):
    @invites_blp.response(200, InviteObject(many=True))
    def get(self):
        """List all invite codes (admin only)"""
        _require_admin()
        from app import InviteCode
        db = current_app.extensions['sqlalchemy']
        invites = db.session.query(InviteCode).all()
        return [
            {
                "id": i.id,
                "code": i.code,
                "status": "Used" if i.used_by else "Active",
                "created_at": i.created_at.timestamp() if i.created_at else None,
                "expires_at": i.expires_at.timestamp() if i.expires_at else None,
                "used_by": i.used_by,
            }
            for i in invites
        ]


@invites_blp.route("/generate")
class InviteGenerateResource(MethodView):
    @invites_blp.response(201, InviteObject)
    def post(self):
        """Generate a new invite code (admin only)"""
        _require_admin()
        from app import InviteCode
        db = current_app.extensions['sqlalchemy']

        new_code = secrets.token_hex(8)
        expiry = datetime.utcnow() + timedelta(days=7)

        invite = InviteCode(
            code=new_code,
            created_by=request.user.id,
            expires_at=expiry,
        )
        db.session.add(invite)
        db.session.commit()

        return {
            "id": invite.id,
            "code": new_code,
            "status": "Active",
            "created_at": invite.created_at.timestamp() if invite.created_at else None,
            "expires_at": expiry.timestamp(),
            "used_by": None,
        }


# ═══════════════════════════════════════════════════════════════════════════
# HELPERS
# ═══════════════════════════════════════════════════════════════════════════

def _require_auth():
    if not hasattr(request, "user") or request.user is None:
        abort(401, message="Authentication required")


def _require_admin():
    _require_auth()
    if not request.user.is_admin:
        abort(403, message="Admin access required")


def _serialize_user(user, domain):
    """Convert a User model to a standardized dict."""
    result = {
        "id": user.id,
        "username": user.username,
        "domain": domain,
        "full_id": f"{user.username}@{domain}",
        "display_name": user.display_name or user.username,
        "profile_pic": user.profile_pic or None,
        "is_admin": user.is_admin,
        "is_2fa_enabled": user.is_2fa_enabled,
    }
    return result


# ═══════════════════════════════════════════════════════════════════════════
# BLUEPRINT REGISTRATION
# ═══════════════════════════════════════════════════════════════════════════

ALL_BLUEPRINTS = [auth_blp, users_blp, chats_blp, messages_blp, uploads_blp, invites_blp]
