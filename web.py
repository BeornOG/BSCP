"""Web UI routes for BSCP"""
from flask import Blueprint, request, render_template, session, redirect, url_for, make_response, abort, current_app
#from app import db, User, UserSession
from datetime import datetime, timedelta
from werkzeug.security import generate_password_hash, check_password_hash
import pyotp
import secrets

web_bp = Blueprint('web', __name__)

@web_bp.route("/")
def index():
    if not hasattr(request, 'user') or request.user is None:
        return redirect(url_for('web.login'))
    from app import DOMAIN
    return render_template('index.html', user=request.user.username, domain=DOMAIN)

@web_bp.route("/login", methods=["GET", "POST"])
def login():
    
    from app import User, UserSession
    db = current_app.extensions['sqlalchemy']
    if request.method == "POST":
        username = request.form.get('user')
        password = request.form.get('password')
        
        user = db.session.query(User).filter_by(username=username).first()

        if user and check_password_hash(user.password_hash, password):
            # If 2FA is enabled, go to 2FA verification
            if user.is_2fa_enabled:
                session['pending_user_id'] = user.id
                return redirect(url_for('web.verify_2fa'))
            else:
                # If 2FA is disabled, create session directly
                device_token = secrets.token_urlsafe(32)
                new_session = UserSession(
                    user_id=user.id,
                    token=device_token,
                    device_info=request.headers.get('User-Agent', 'Unknown Device'),
                    expires_at=datetime.utcnow() + timedelta(days=30)
                )
                db.session.add(new_session)
                db.session.commit()

                session.clear()
                session['session_token'] = device_token
                return redirect(url_for('web.index'))
            
    return render_template('login.html')

@web_bp.route("/login/2fa", methods=["GET", "POST"])
def verify_2fa():
    from app import User, UserSession
    db = current_app.extensions['sqlalchemy']
    user_id = session.get('pending_user_id')
    if not user_id:
        return redirect(url_for('web.login'))

    user = db.session.query(User).get(user_id)

    if request.method == "POST":
        otp_code = request.form.get('otp')
        totp = pyotp.TOTP(user.otp_secret)

        if totp.verify(otp_code):
            # 2FA geslaagd: Maak een uniek device token
            device_token = secrets.token_urlsafe(32)
            new_session = UserSession(
                user_id=user.id,
                token=device_token,
                device_info=request.headers.get('User-Agent', 'Unknown Device'),
                expires_at=datetime.utcnow() + timedelta(days=30)
            )
            db.session.add(new_session)
            db.session.commit()

            # Sessie definitief maken
            session.clear()
            session['session_token'] = device_token
            return redirect(url_for('web.index'))

    return render_template('2fa.html') # Formulier met 1 input voor de code

@web_bp.route("/setup", methods=["GET", "POST"])
def setup():
    """First-time setup: create initial admin account"""
    from app import User, UserSession
    db = current_app.extensions['sqlalchemy']

    # Check if setup is already complete
    user_count = db.session.query(User).count()
    if user_count > 0:
        return redirect(url_for('web.login'))

    if request.method == "POST":
        username = request.form.get('username', '').strip()
        password = request.form.get('password', '')
        password_confirm = request.form.get('password_confirm', '')
        email = request.form.get('email', '').strip() or None

        # Validation
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
            return render_template('setup.html', errors=errors)

        # Create admin user
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

        return redirect(url_for('web.login'))

    return render_template('setup.html')

@web_bp.route("/register", methods=["GET", "POST"])
def register():
    """User registration with invite code"""
    from app import User, UserSession, InviteCode
    db = current_app.extensions['sqlalchemy']

    # Check if setup is complete
    user_count = db.session.query(User).count()
    if user_count == 0:
        return redirect(url_for('web.setup'))

    if request.method == "POST":
        username = request.form.get('username', '').strip()
        password = request.form.get('password', '')
        password_confirm = request.form.get('password_confirm', '')
        invite_code = request.form.get('invite_code', '').strip()

        # Validation
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

        # Validate invite code if no other errors
        if not errors:
            invite = db.session.query(InviteCode).filter_by(code=invite_code).first()
            if not invite:
                errors.append("Invalid invite code")
            elif invite.used_by is not None:
                errors.append("This invite code has already been used")
            elif invite.expires_at and invite.expires_at < datetime.utcnow():
                errors.append("This invite code has expired")

        if errors:
            return render_template('register.html', errors=errors)

        # Create user
        user = User(
            username=username,
            password_hash=generate_password_hash(password)
        )
        db.session.add(user)
        db.session.flush()  # Get the user ID

        # Mark invite code as used
        invite.used_by = user.id
        invite.used_at = datetime.utcnow()
        db.session.commit()

        return redirect(url_for('web.login'))

    return render_template('register.html')

@web_bp.before_app_request
def load_logged_in_user():
    from app import User, UserSession
    db = current_app.extensions['sqlalchemy']
    token = session.get('session_token')
    if token:
        user_session = db.session.query(UserSession).filter_by(token=token).first()
        if user_session and user_session.expires_at > datetime.utcnow():
            # Update last active voor dit apparaat
            user_session.last_active = datetime.utcnow()
            db.session.commit()
            request.user = user_session.user
        else:
            session.clear()
            request.user = None
    else:
        request.user = None

def login_required(f):
    from app import User, UserSession
    from functools import wraps
    db = current_app.extensions['sqlalchemy']
    @wraps(f)
    def decorated_function(*args, **kwargs):
        if request.user is None:
            return redirect(url_for('web.login'))
        return f(*args, **kwargs)
    return decorated_function