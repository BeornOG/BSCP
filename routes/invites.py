"""Invite code routes — /api/invites"""
from flask.views import MethodView
from flask_smorest import Blueprint as SmorestBlueprint
from flask import request, current_app
from datetime import datetime, timedelta
import secrets

from schemas import InviteObject
from routes import require_admin


invites_blp = SmorestBlueprint("invites", __name__, url_prefix="/api/invites",
                                description="Invite code management (admin)")


@invites_blp.route("/")
class InviteListResource(MethodView):
    @invites_blp.response(200, InviteObject(many=True))
    def get(self):
        """List all invite codes (admin only)"""
        require_admin()
        from app import InviteCode
        db = current_app.extensions['sqlalchemy']
        invites = db.session.query(InviteCode).all()
        return [_serialize_invite(i) for i in invites]


@invites_blp.route("/generate")
class InviteGenerateResource(MethodView):
    @invites_blp.response(201, InviteObject)
    def post(self):
        """Generate a new invite code (admin only)"""
        require_admin()
        from app import InviteCode
        db = current_app.extensions['sqlalchemy']

        new_code = secrets.token_hex(8)
        expiry = datetime.utcnow() + timedelta(days=7)

        invite = InviteCode(
            code=new_code,
            created_by=request.user.id,
            expires_at=expiry,
        )
        db.session.add(invite)
        db.session.commit()

        return _serialize_invite(invite)


def _serialize_invite(invite):
    """Convert an InviteCode model to a response dict."""
    return {
        "id": invite.id,
        "code": invite.code,
        "status": "Used" if invite.used_by else "Active",
        "created_at": invite.created_at.timestamp() if invite.created_at else None,
        "expires_at": invite.expires_at.timestamp() if invite.expires_at else None,
        "used_by": invite.used_by,
    }
