"""Web UI routes for BSCP"""
from flask import Blueprint, request, session, redirect, url_for, current_app, jsonify
from datetime import datetime, timedelta
from werkzeug.security import generate_password_hash, check_password_hash
import pyotp
import secrets
import uuid

web_bp = Blueprint('web', __name__)

# --- AUTH API ENDPOINTS (JSON) ---

@web_bp.route("/api/auth/login", methods=["POST"])
def api_login():
    from app import User, UserSession
    db = current_app.extensions['sqlalchemy']

    data = request.get_json(silent=True)
    if data:
        username = data.get('user', '')
        password = data.get('password', '')
    else:
        username = request.form.get('user', '')
        password = request.form.get('password', '')

    user = db.session.query(User).filter_by(username=username).first()

    if not user or not check_password_hash(user.password_hash, password):
        return jsonify({"error": "Invalid username or password"}), 401

    if user.is_2fa_enabled:
        session['pending_user_id'] = user.id
        return jsonify({"requires_2fa": True}), 200

    device_token = secrets.token_urlsafe(32)
    new_session = UserSession(
        id=str(uuid.uuid4()),
        user_id=user.id,
        token=device_token,
        device_info=request.headers.get('User-Agent', 'Unknown Device'),
        expires_at=datetime.utcnow() + timedelta(days=30)
    )
    db.session.add(new_session)
    db.session.commit()

    session.clear()
    session['session_token'] = device_token
    return jsonify({"success": True}), 200


@web_bp.route("/api/auth/2fa", methods=["POST"])
def api_verify_2fa():
    from app import User, UserSession
    db = current_app.extensions['sqlalchemy']
    user_id = session.get('pending_user_id')
    if not user_id:
        return jsonify({"error": "No pending 2FA session"}), 400

    user = db.session.query(User).get(user_id)
    if not user:
        return jsonify({"error": "User not found"}), 400

    data = request.get_json(silent=True)
    otp_code = data.get('otp', '') if data else request.form.get('otp', '')

    totp = pyotp.TOTP(user.otp_secret)
    if not totp.verify(otp_code):
        return jsonify({"error": "Invalid code"}), 401

    device_token = secrets.token_urlsafe(32)
    new_session = UserSession(
        user_id=user.id,
        token=device_token,
        device_info=request.headers.get('User-Agent', 'Unknown Device'),
        expires_at=datetime.utcnow() + timedelta(days=30),
        id=str(uuid.uuid4())
    )
    db.session.add(new_session)
    db.session.commit()

    session.clear()
    session['session_token'] = device_token
    return jsonify({"success": True}), 200


@web_bp.route("/api/auth/setup", methods=["GET", "POST"])
def api_setup():
    """First-time setup: create initial admin account"""
    from app import User
    db = current_app.extensions['sqlalchemy']

    user_count = db.session.query(User).count()

    if request.method == "GET":
        return jsonify({"needs_setup": user_count == 0}), 200

    if user_count > 0:
        return jsonify({"error": "Setup already complete"}), 400

    data = request.get_json(silent=True)
    if data:
        username = data.get('username', '').strip()
        password = data.get('password', '')
        password_confirm = data.get('password_confirm', '')
        email = data.get('email', '').strip() or None
    else:
        username = request.form.get('username', '').strip()
        password = request.form.get('password', '')
        password_confirm = request.form.get('password_confirm', '')
        email = request.form.get('email', '').strip() or None

    errors = []
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
        return jsonify({"errors": errors}), 400

    user = User(
        username=username,
        password_hash=generate_password_hash(password),
        email=email,
        is_admin=True,
        otp_secret=pyotp.random_base32(),
        is_2fa_enabled=False
    )
    db.session.add(user)
    db.session.commit()

    return jsonify({"success": True}), 201


@web_bp.route("/api/auth/register", methods=["POST"])
def api_register():
    """User registration with invite code"""
    from app import User, InviteCode
    db = current_app.extensions['sqlalchemy']

    user_count = db.session.query(User).count()
    if user_count == 0:
        return jsonify({"error": "Setup required first"}), 400

    data = request.get_json(silent=True)
    if data:
        username = data.get('username', '').strip()
        password = data.get('password', '')
        password_confirm = data.get('password_confirm', '')
        invite_code = data.get('invite_code', '').strip()
    else:
        username = request.form.get('username', '').strip()
        password = request.form.get('password', '')
        password_confirm = request.form.get('password_confirm', '')
        invite_code = request.form.get('invite_code', '').strip()

    errors = []
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
            errors.append("This invite code has already been used")
        elif invite.expires_at and invite.expires_at < datetime.utcnow():
            errors.append("This invite code has expired")

    if errors:
        return jsonify({"errors": errors}), 400

    user = User(
        id=str(uuid.uuid4()),
        username=username,
        password_hash=generate_password_hash(password)
    )
    db.session.add(user)
    db.session.flush()

    invite.used_by = user.id
    invite.used_at = datetime.utcnow()
    db.session.commit()

    return jsonify({"success": True}), 201


@web_bp.route("/api/auth/logout", methods=["POST"])
def api_logout():
    from app import UserSession
    db = current_app.extensions['sqlalchemy']
    token = session.get('session_token')
    if token:
        user_session = db.session.query(UserSession).filter_by(token=token).first()
        if user_session:
            db.session.delete(user_session)
            db.session.commit()
    session.clear()
    return jsonify({"success": True}), 200


# --- ADMIN API ENDPOINTS ---

@web_bp.route("/api/users", methods=["GET"])
def get_users():
    from app import User
    db = current_app.extensions['sqlalchemy']
    if not hasattr(request, 'user') or not request.user or not request.user.is_admin:
        return jsonify({"error": "Unauthorized"}), 401
    users = db.session.query(User).all()
    return jsonify([{
        "id": u.id,
        "username": u.username,
        "is_admin": u.is_admin,
        "is_2fa": u.is_2fa_enabled
    } for u in users])

@web_bp.route("/api/users/<int:user_id>", methods=["DELETE"])
def delete_user(user_id):
    from app import User
    db = current_app.extensions['sqlalchemy']
    if not hasattr(request, 'user') or not request.user or not request.user.is_admin:
        return jsonify({"error": "Unauthorized"}), 401

    user = db.session.query(User).get_or_404(user_id)

    if user.is_admin:
        return jsonify({"error": "Cannot delete admin user"}), 400

    user.is_deleted = True
    user.sessions = []
    db.session.commit()
    return jsonify({"message": f"User {user.username} has been deactivated."})

@web_bp.route("/api/invites", methods=["GET"])
def get_invites():
    if not hasattr(request, 'user') or not request.user or not request.user.is_admin:
        return jsonify({"error": "Unauthorized"}), 401
    from app import InviteCode
    db = current_app.extensions['sqlalchemy']
    invites = db.session.query(InviteCode).all()
    return jsonify([{
        "code": i.code,
        "created_by": i.created_by,
        "used_by": i.used_by,
        "status": "Used" if i.used_by else "Active",
        "expires_at": i.expires_at.timestamp() if i.expires_at else "Never"
    } for i in invites])

@web_bp.route("/api/invites/generate", methods=["POST"])
def generate_invite():
    from app import InviteCode
    if not hasattr(request, 'user') or not request.user or not request.user.is_admin:
        return jsonify({"error": "Unauthorized"}), 401
    db = current_app.extensions['sqlalchemy']

    creator_id = 1
    new_code = secrets.token_hex(8)
    expiry = datetime.utcnow() + timedelta(days=7)

    invite = InviteCode(
        code=new_code,
        created_by=creator_id,
        expires_at=expiry
    )
    db.session.add(invite)
    db.session.commit()

    return jsonify({"code": new_code, "expires_at": expiry.strftime("%Y-%m-%d")})


# --- SESSION MIDDLEWARE ---

@web_bp.before_app_request
def load_logged_in_user():
    from app import User, UserSession
    db = current_app.extensions['sqlalchemy']
    token = session.get('session_token')
    if token:
        user_session = db.session.query(UserSession).filter_by(token=token).first()
        if user_session and user_session.expires_at > datetime.utcnow():
            user_session.last_active = datetime.utcnow()
            db.session.commit()
            request.user = user_session.user
        else:
            session.clear()
            request.user = None
    else:
        request.user = None
