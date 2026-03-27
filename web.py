"""Web UI routes for BSCP"""
from flask import Blueprint, request, render_template, session, redirect, url_for, make_response, abort
#from app import db, User, UserSession
from datetime import datetime, timedelta
from werkzeug.security import generate_password_hash, check_password_hash
import pyotp
import secrets

web_bp = Blueprint('web', __name__)

@web_bp.route("/")
def index():
    from app import db, User, UserSession
    if 'username' not in session: return redirect(url_for('web.login'))
    return render_template('index.html', user=session['username'])

@web_bp.route("/login", methods=["GET", "POST"])
def login():
    from app import db, User, UserSession
    if request.method == "POST":
        username = request.form.get('user')
        password = request.form.get('password')
        
        user = User.query.filter_by(username=username).first()
        
        if user and check_password_hash(user.password_hash, password):
            # Stap 1 geslaagd: Sla user_id tijdelijk op in de Flask-sessie
            session['pending_user_id'] = user.id
            return redirect(url_for('web.verify_2fa'))
            
    return render_template('login.html')

@web_bp.route("/login/2fa", methods=["GET", "POST"])
def verify_2fa():
    from app import db, User, UserSession
    user_id = session.get('pending_user_id')
    if not user_id:
        return redirect(url_for('web.login'))

    user = User.query.get(user_id)

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

@web_bp.before_app_request
def load_logged_in_user():
    from app import db, User, UserSession
    token = session.get('session_token')
    if token:
        user_session = UserSession.query.filter_by(token=token).first()
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
    from app import db, User, UserSession
    from functools import wraps
    @wraps(f)
    def decorated_function(*args, **kwargs):
        if request.user is None:
            return redirect(url_for('web.login'))
        return f(*args, **kwargs)
    return decorated_function