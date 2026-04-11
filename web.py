"""Session middleware for BSCP — loads authenticated user on each request."""
from flask import Blueprint, request, session, current_app
from datetime import datetime

web_bp = Blueprint('web', __name__)


@web_bp.before_app_request
def load_logged_in_user():
    # Skip session lookup for static files and non-API routes
    path = request.path
    if not (path.startswith("/api/") or path.startswith("/federation/")
            or path.startswith("/media/") or path.startswith("/uploads/")):
        request.user = None
        return

    from app import UserSession
    db = current_app.extensions['sqlalchemy']
    token = session.get('session_token')
    if not token:
        request.user = None
        return

    from app import UserSession
    db = current_app.extensions['sqlalchemy']
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
    else:
        # Expired or not found — just treat as unauthenticated.
        # Never call session.clear() here; only the logout endpoint
        # should wipe the cookie. Concurrent requests on page load
        # could race and clear a valid session otherwise.
        request.user = None
