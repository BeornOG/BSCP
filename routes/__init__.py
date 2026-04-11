"""API routes package — shared helpers and blueprint registry."""
from flask import request
from flask_smorest import abort


def require_auth():
    """Abort 401 if no authenticated user on the request."""
    if not hasattr(request, "user") or request.user is None:
        abort(401, message="Authentication required")


def require_admin():
    """Abort 403 if authenticated user is not an admin."""
    require_auth()
    if not request.user.is_admin:
        abort(403, message="Admin access required")


# -- Blueprint registry (import order matters: new modules first) -----------
from routes.auth import auth_blp                                        # noqa: E402
from routes.users import users_blp                                      # noqa: E402
from routes.chats import chats_blp                                      # noqa: E402
from routes.uploads import uploads_blp                                  # noqa: E402
from routes.invites import invites_blp                                  # noqa: E402

ALL_BLUEPRINTS = [auth_blp, users_blp, chats_blp, uploads_blp, invites_blp]
