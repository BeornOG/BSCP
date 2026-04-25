"""Admin routes — /api/admin"""
from flask.views import MethodView
from flask_smorest import Blueprint as SmorestBlueprint, abort
from flask import current_app, request
from marshmallow import ValidationError

from schemas import ServerConfigResponse, ServerConfigUpdate
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
        """Update server configuration (admin only)"""
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
