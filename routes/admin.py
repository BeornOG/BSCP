"""Admin routes — /api/admin"""
from flask.views import MethodView
from flask_smorest import Blueprint as SmorestBlueprint, abort
from flask import current_app, request
from marshmallow import ValidationError

from schemas import ServerConfigResponse, ServerConfigUpdate, UserStorageConfigResponse, UserStorageConfigUpdate
from routes import require_auth, require_admin


admin_blp = SmorestBlueprint("admin", __name__, url_prefix="/api/admin",
                              description="Admin configuration")


@admin_blp.route("/config")
class AdminConfigResource(MethodView):
    @admin_blp.response(200, ServerConfigResponse)
    def get(self):
        """Get server configuration (admin only)"""
        require_admin()
        from app import ServerConfig
        db = current_app.extensions['sqlalchemy']

        config = db.session.query(ServerConfig).first()
        if not config:
            config = ServerConfig()
            db.session.add(config)
            db.session.commit()

        return {"storage_limit_mb": config.storage_limit_mb}

    @admin_blp.arguments(ServerConfigUpdate)
    @admin_blp.response(200, ServerConfigResponse)
    def patch(self, args):
        """Update default storage limit for new users (admin only)"""
        require_admin()
        from app import ServerConfig
        db = current_app.extensions['sqlalchemy']

        config = db.session.query(ServerConfig).first()
        if not config:
            config = ServerConfig()
            db.session.add(config)

        if "storage_limit_mb" in args:
            if args["storage_limit_mb"] < 1:
                abort(400, message="Storage limit must be at least 1 MB")
            config.storage_limit_mb = args["storage_limit_mb"]

        db.session.commit()
        return {"storage_limit_mb": config.storage_limit_mb}


@admin_blp.route("/users/<username>/storage")
class UserStorageConfigResource(MethodView):
    @admin_blp.response(200, UserStorageConfigResponse)
    def get(self, username):
        """Get user's storage limit (admin only)"""
        require_admin()
        from app import User
        db = current_app.extensions['sqlalchemy']

        # Extract local username if federated (user@domain -> user)
        local_username = username.split('@')[0] if '@' in username else username

        user = db.session.query(User).filter_by(username=local_username).first()
        if not user:
            abort(404, message="User not found")

        return {
            "user_id": user.id,
            "username": user.username,
            "storage_limit_mb": user.storage_limit_mb,
        }

    @admin_blp.arguments(UserStorageConfigUpdate)
    @admin_blp.response(200, UserStorageConfigResponse)
    def patch(self, args, username):
        """Update user's storage limit (admin only)"""
        require_admin()
        from app import User
        db = current_app.extensions['sqlalchemy']

        # Extract local username if federated (user@domain -> user)
        local_username = username.split('@')[0] if '@' in username else username

        user = db.session.query(User).filter_by(username=local_username).first()
        if not user:
            abort(404, message="User not found")

        if "storage_limit_mb" in args:
            limit = args["storage_limit_mb"]
            if limit < 1:
                abort(400, message="Storage limit must be at least 1 MB")
            user.storage_limit_mb = limit

        db.session.commit()
        return {
            "user_id": user.id,
            "username": user.username,
            "storage_limit_mb": user.storage_limit_mb,
        }
