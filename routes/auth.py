"""Authentication routes — /api/auth"""
from flask.views import MethodView
from flask_smorest import Blueprint as SmorestBlueprint, abort
from flask import request, session, current_app
from datetime import datetime, timedelta
from werkzeug.security import generate_password_hash, check_password_hash
import pyotp, secrets, uuid

from schemas import (
    LoginRequest, LoginResponse, TwoFactorRequest,
    SetupRequest, SetupStatusResponse, RegisterRequest,
    AuthSuccessResponse, AuthErrorResponse,
)


auth_blp = SmorestBlueprint("auth", __name__, url_prefix="/api/auth",
                             description="Authentication & account management")


# ── /setup ────────────────────────────────────────────────────────────────

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
            is_primary_admin=True,
            otp_secret=pyotp.random_base32(),
            is_2fa_enabled=False,
        )
        db.session.add(user)
        db.session.commit()
        return {"success": True}


# ── /login ────────────────────────────────────────────────────────────────

@auth_blp.route("/login")
class LoginResource(MethodView):
    @auth_blp.arguments(LoginRequest)
    @auth_blp.response(200, LoginResponse)
    def post(self, data):
        """Authenticate with username and password"""
        from app import User
        db = current_app.extensions['sqlalchemy']

        user = db.session.query(User).filter_by(username=data["user"]).first()
        if not user or not check_password_hash(user.password_hash, data["password"]):
            return {"success": False, "error": "Invalid username or password"}

        if user.is_2fa_enabled:
            session["pending_user_id"] = user.id
            return {"success": False, "requires_2fa": True}

        device_token = _create_session(db, user)
        return {"success": True, "session_token": device_token}


# ── /2fa ──────────────────────────────────────────────────────────────────

@auth_blp.route("/2fa")
class TwoFactorResource(MethodView):
    @auth_blp.arguments(TwoFactorRequest)
    @auth_blp.response(200, AuthSuccessResponse)
    def post(self, data):
        """Verify 2FA one-time code"""
        from app import User
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

        device_token = _create_session(db, user)
        return {"success": True, "session_token": device_token}


# ── /register ─────────────────────────────────────────────────────────────

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


# ── /logout ───────────────────────────────────────────────────────────────

@auth_blp.route("/logout")
class LogoutResource(MethodView):
    @auth_blp.response(200, AuthSuccessResponse)
    def post(self):
        """Log out and destroy session"""
        from app import UserSession
        db = current_app.extensions['sqlalchemy']
        token = session.get("session_token")
        if token:
            from app import User
            us = db.session.query(UserSession).filter_by(token=token).first()
            if us:
                user_id = us.user_id
                db.session.delete(us)
                db.session.commit()
                remaining = db.session.query(UserSession).filter(
                    UserSession.user_id == user_id,
                    UserSession.expires_at > datetime.utcnow()
                ).count()
                if remaining == 0:
                    user = db.session.query(User).get(user_id)
                    if user:
                        user.Status_type = 1
                        db.session.commit()
        session.clear()
        return {"success": True}


# ── helpers ───────────────────────────────────────────────────────────────

def _create_session(db, user):
    """Create a new device session and store the token in the flask session."""
    from app import UserSession
    device_token = secrets.token_urlsafe(32)
    if user.Status_type not in (2, 3):
        user.Status_type = 0
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
    return device_token
