"""Webhook management routes — /api/user/webhooks"""
from flask.views import MethodView
from flask_smorest import Blueprint as SmorestBlueprint, abort
from flask import request, current_app
from datetime import datetime
import uuid
import secrets

from schemas import WebhookObject, WebhookCreateRequest, WebhookRegenerateResponse, WebhookPayload
from routes import require_auth


webhooks_blp = SmorestBlueprint("webhooks", __name__, url_prefix="/api/user/webhooks",
                                description="Personal webhook management")


# ── / (list & create webhooks) ────────────────────────────────────────────

@webhooks_blp.route("/")
class WebhookListResource(MethodView):
    @webhooks_blp.response(200, WebhookObject(many=True))
    def get(self):
        """List user's webhooks"""
        require_auth()
        from app import Webhook, DOMAIN
        db = current_app.extensions['sqlalchemy']

        webhooks = db.session.query(Webhook).filter_by(user_id=request.user.id).all()

        return [_serialize_webhook(w, DOMAIN) for w in webhooks]

    @webhooks_blp.arguments(WebhookCreateRequest)
    @webhooks_blp.response(201, WebhookObject)
    def post(self, data):
        """Create new webhook. Name is immutable and used as message sender identity."""
        require_auth()
        from app import Webhook, DOMAIN
        db = current_app.extensions['sqlalchemy']

        webhook = Webhook(
            user_id=request.user.id,
            name=data["name"],
            token=secrets.token_urlsafe(32),
            profile_pic=data.get("avatar_url"),
        )
        db.session.add(webhook)
        db.session.commit()

        return _serialize_webhook(webhook, DOMAIN)


# ── /{webhook_id} (delete webhook) ────────────────────────────────────────

@webhooks_blp.route("/<webhook_id>")
class WebhookDetailResource(MethodView):
    @webhooks_blp.response(204)
    def delete(self, webhook_id):
        """Delete a webhook"""
        require_auth()
        from app import Webhook
        db = current_app.extensions['sqlalchemy']

        webhook = db.session.query(Webhook).filter_by(id=webhook_id, user_id=request.user.id).first()
        if not webhook:
            abort(404, message="Webhook not found")

        db.session.delete(webhook)
        db.session.commit()
        return None


# ── /{webhook_id}/regenerate ────────────────────────────────────────────────

@webhooks_blp.route("/<webhook_id>/regenerate", methods=["POST"])
class WebhookRegenerateResource(MethodView):
    @webhooks_blp.response(200, WebhookRegenerateResponse)
    def post(self, webhook_id):
        """Regenerate webhook token"""
        require_auth()
        from app import Webhook, DOMAIN
        db = current_app.extensions['sqlalchemy']

        webhook = db.session.query(Webhook).filter_by(id=webhook_id, user_id=request.user.id).first()
        if not webhook:
            abort(404, message="Webhook not found")

        webhook.token = secrets.token_urlsafe(32)
        db.session.commit()

        return {"url": _get_webhook_url(webhook, DOMAIN)}


def _serialize_webhook(webhook, domain):
    """Convert Webhook model to response dict."""
    return {
        "id": webhook.id,
        "name": webhook.name,
        "url": _get_webhook_url(webhook, domain),
        "profile_pic": webhook.profile_pic,
        "created_at": webhook.created_at.timestamp(),
        "last_used": webhook.last_used.timestamp() if webhook.last_used else None,
    }


def _get_webhook_url(webhook, domain):
    """Generate full webhook URL."""
    return f"http://{domain}/webhooks/{webhook.id}/{webhook.token}"
