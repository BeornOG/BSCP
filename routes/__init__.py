"""API routes package — shared helpers and blueprint registry."""
from datetime import datetime, timedelta
from flask import request
from flask_smorest import abort

ONLINE_THRESHOLD = timedelta(seconds=5)
AWAY_THRESHOLD = timedelta(hours=1)
INACTIVE_SESSION_THRESHOLD = timedelta(hours=6)
STATUS_MAP = {0: "online", 1: "offline", 2: "away", 3: "dnd"}


def require_auth():
    """Abort 401 if no authenticated user on the request."""
    if not hasattr(request, "user") or request.user is None:
        abort(401, message="Authentication required")


def require_admin():
    """Abort 403 if authenticated user is not an admin."""
    require_auth()
    if not request.user.is_admin:
        abort(403, message="Admin access required")


def get_user_status(user) -> str:
    """Return the status string for a User model instance."""
    now = datetime.utcnow()
    sessions = [
        s for s in getattr(user, "sessions", [])
        if s.expires_at and s.expires_at > now
           and s.last_active
           and now - s.last_active <= INACTIVE_SESSION_THRESHOLD
    ]
    if not sessions:
        return "offline"

    if user.Status_type in (2, 3):
        return STATUS_MAP.get(user.Status_type, "away")
    if user.Status_type == 1:
        return "offline"

    last_active = max(s.last_active for s in sessions)
    if now - last_active <= ONLINE_THRESHOLD:
        return "online"
    if now - last_active <= AWAY_THRESHOLD:
        return "away"
    return "offline"


# -- Blueprint registry (import order matters: new modules first) -----------
from routes.auth import auth_blp                                        # noqa: E402
from routes.users import users_blp                                      # noqa: E402
from routes.chats import chats_blp                                      # noqa: E402
from routes.uploads import uploads_blp                                  # noqa: E402
from routes.invites import invites_blp                                  # noqa: E402

ALL_BLUEPRINTS = [auth_blp, users_blp, chats_blp, uploads_blp, invites_blp]
