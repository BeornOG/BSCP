"""Chat and message routes — /api/chats"""
from flask.views import MethodView
from flask_smorest import Blueprint as SmorestBlueprint, abort
from flask import request, current_app
from datetime import datetime
import uuid
from sqlalchemy import or_

from schemas import ChatObject, MessageObject, MessagesQueryArgs, SendMessageBody
from routes import require_auth
from services.users import get_profile


chats_blp = SmorestBlueprint("chats", __name__, url_prefix="/api/chats",
                              description="Chat conversations and messages")


# ── / (conversation list) ────────────────────────────────────────────────

@chats_blp.route("/")
class ChatListResource(MethodView):
    @chats_blp.response(200, ChatObject(many=True))
    def get(self):
        """List all conversations for the authenticated user"""
        require_auth()
        from app import Message, DOMAIN, Webhook
        db = current_app.extensions['sqlalchemy']
        me = f"{request.user.username}@{DOMAIN}"

        sent_to = set(r[0] for r in
            db.session.query(Message.receiver).filter(Message.sender == me).distinct())
        received_from = set(r[0] for r in
            db.session.query(Message.sender).filter(Message.receiver == me).distinct())

        partners = sent_to | received_from
        chats = []

        for partner in sorted(partners):
            display_name = partner.split("@")[0]
            profile_pic = None
            status = "offline"

            webhook = None
            if partner.endswith(f"@{DOMAIN}") and partner.startswith("webhook-"):
                webhook_id = partner.split("@")[0][8:]  # Remove "webhook-" prefix
                webhook = db.session.query(Webhook).filter_by(id=webhook_id, user_id=request.user.id).first()

            if webhook:
                display_name = webhook.name
                profile_pic = webhook.profile_pic
            else:
                try:
                    profile = get_profile(partner)
                except ConnectionError:
                    profile = None

                if profile:
                    display_name = profile["display_name"]
                    profile_pic = profile["profile_pic"]
                    status = profile["status"]

            unread_count = db.session.query(Message).filter(
                Message.sender == partner,
                Message.receiver == me,
                Message.is_read == False
            ).count()

            last_msg = db.session.query(Message).filter(
                or_(
                    (Message.sender == partner) & (Message.receiver == me),
                    (Message.sender == me) & (Message.receiver == partner)
                )
            ).order_by(Message.timestamp.desc()).first()

            chats.append({
                "id": partner,
                "display_name": display_name,
                "profile_pic": profile_pic,
                "status": status,
                "unread_count": unread_count,
                "last_message_text": last_msg.text if last_msg else None,
                "last_message_sender": last_msg.sender if last_msg else None,
            })

        return chats


# ── /<target>/messages ────────────────────────────────────────────────────

@chats_blp.route("/<path:target>/messages")
class ChatMessagesResource(MethodView):
    @chats_blp.arguments(MessagesQueryArgs, location="query")
    @chats_blp.response(200, MessageObject(many=True))
    def get(self, args, target):
        """Get messages for a conversation"""
        require_auth()
        from app import Message, DOMAIN
        from json_discovery import get_endpoint
        from sqlalchemy import or_
        import requests as http_requests

        db = current_app.extensions['sqlalchemy']
        me = f"{request.user.username}@{DOMAIN}"

        # Channel server (external)
        if "#" in target:
            target_domain = target.split("#")[0]
            url = get_endpoint(target_domain, "channelserver", "channel_poll")
            if not url:
                url = f"http://{target_domain}/api/channel/poll"
            params = {"path": target, "limit": args["limit"], "since": args.get("since"), "before": args.get("before")}
            try:
                return http_requests.get(url, params=params).json()
            except Exception:
                return []

        target_full = f"{target}@{DOMAIN}" if "@" not in target else target

        sent = (Message.sender == me) & (Message.receiver == target_full)
        received = (Message.sender == target_full) & (Message.receiver == me)
        query = db.session.query(Message).filter(or_(sent, received))

        if args.get("since"):
            query = query.filter(Message.timestamp > datetime.fromtimestamp(args["since"]))
        if args.get("before"):
            query = query.filter(Message.timestamp < datetime.fromtimestamp(args["before"]))

        msgs = query.order_by(Message.timestamp.desc()).limit(args["limit"]).all()

        if target_full and target_full != me:
            unread_msgs = db.session.query(Message).filter(
                Message.sender == target_full,
                Message.receiver == me,
                Message.is_read == False
            ).all()
            if unread_msgs:
                for m in unread_msgs:
                    m.is_read = True
                db.session.commit()

        return [_serialize_message(m) for m in reversed(msgs)]

    @chats_blp.arguments(SendMessageBody)
    @chats_blp.response(201, MessageObject)
    def post(self, data, target):
        """Send a message to a conversation"""
        require_auth()
        from app import Message as MsgModel, DOMAIN
        from json_discovery import get_endpoint
        import requests as http_requests

        db = current_app.extensions['sqlalchemy']

        receiver = f"{target}@{DOMAIN}" if "@" not in target else target
        sender = f"{request.user.username}@{DOMAIN}"

        # Block sending to webhooks (one-way only)
        username = receiver.split("@")[0]
        if username.startswith("webhook-"):
            abort(403, message="Cannot send messages to webhooks")

        # Verify the recipient exists before saving
        if "#" not in target:
            try:
                profile = get_profile(receiver)
            except ConnectionError:
                abort(502, message="Failed to reach remote server")
            if not profile:
                abort(404, message="User not found")

        msg_uuid = str(uuid.uuid4())
        full_id = f"{DOMAIN}/{msg_uuid}"
        val_key = "key-" + msg_uuid[:8]

        new_msg = MsgModel(
            id=full_id,
            sender=sender,
            receiver=receiver,
            text=data["text"],
            validation_key=val_key,
        )
        db.session.add(new_msg)
        db.session.commit()

        if receiver.endswith(f"@{DOMAIN}"):
            from app import User, send_push_notification
            recipient = db.session.query(User).filter_by(username=receiver.split("@", 1)[0]).first()
            if recipient and recipient.id != request.user.id:
                send_push_notification(
                    recipient,
                    f"New message from {request.user.username}",
                    data["text"],
                    url='/',
                )

        # Federate
        msg_payload = {
            "id": full_id,
            "sender": sender,
            "receiver": receiver,
            "text": data["text"],
            "validationKey": val_key,
        }

        if "#" in target:
            target_domain = target.split("#")[0]
            url = get_endpoint(target_domain, "channelserver", "channel_send")
            if not url:
                url = f"http://{target_domain}/api/channel/send"
            try:
                http_requests.post(url, json=msg_payload, timeout=3)
            except Exception:
                pass
        else:
            target_domain = receiver.split("@")[-1]
            url = get_endpoint(target_domain, "userserver", "federation_receive")
            if not url:
                url = f"http://{target_domain}/federation/receive"
            try:
                http_requests.post(url, json=msg_payload, timeout=5)
            except Exception:
                pass

        return _serialize_message(new_msg)


def _serialize_message(msg):
    """Convert a Message model to a response dict."""
    return {
        "id": msg.id,
        "sender": msg.sender,
        "receiver": msg.receiver,
        "text": msg.text,
        "timestamp": msg.timestamp.timestamp(),
        "is_read": msg.is_read,
    }
