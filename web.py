"""Session middleware for BSCP — loads authenticated user on each request."""
from flask import Blueprint, request, session, current_app
from datetime import datetime

web_bp = Blueprint('web', __name__)


@web_bp.before_app_request
def load_logged_in_user():
    from app import UserSession
    db = current_app.extensions['sqlalchemy']
    token = session.get('session_token')
    if token:
        try:
            user_session = db.session.query(UserSession).filter_by(token=token).first()
        except Exception:
            db.session.rollback()
            request.user = None
            return
        if user_session and user_session.expires_at > datetime.utcnow():
            try:
                user_session.last_active = datetime.utcnow()
                db.session.commit()
            except Exception:
                db.session.rollback()
            request.user = user_session.user
        elif user_session:
            # Session exists but is expired — clear it
            session.clear()
            request.user = None
        else:
            # Token not found in DB — clear it
            session.clear()
            request.user = None
    else:
        request.user = None
