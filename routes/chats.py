"""Chat and message routes — /api/chats"""
from flask.views import MethodView
from flask_smorest import Blueprint as SmorestBlueprint, abort
from flask import request, current_app
from datetime import datetime
import uuid

from schemas import ChatObject, MessageObject, MessagesQueryArgs, SendMessageBody
from routes import require_auth, get_user_status


chats_blp = SmorestBlueprint("chats", __name__, url_prefix="/api/chats",
                              description="Chat conversations and messages")


# ── / (conversation list) ────────────────────────────────────────────────

@chats_blp.route("/")
class ChatListResource(MethodView):
    @chats_blp.response(200, ChatObject(many=True))
    def get(self):
        """List all conversations for the authenticated user"""
        require_auth()
        from app import Message, User, DOMAIN
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
            if "@" in partner:
                uname, domain = partner.split("@", 1)
                if domain == DOMAIN:
                    u = db.session.query(User).filter_by(username=uname).first()
                    if u:
                        if u.display_name:
                            display_name = u.display_name
                        profile_pic = u.profile_pic
                        status = get_user_status(u)

            chats.append({"id": partner, "display_name": display_name, "profile_pic": profile_pic, "status": status})

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

        return [_serialize_message(m) for m in reversed(msgs)]

    @chats_blp.arguments(SendMessageBody)
    @chats_blp.response(201, MessageObject)
    def post(self, data, target):
        """Send a message to a conversation"""
        require_auth()
        from app import Message as MsgModel, User, DOMAIN
        from json_discovery import get_endpoint
        import requests as http_requests

        db = current_app.extensions['sqlalchemy']

        receiver = f"{target}@{DOMAIN}" if "@" not in target else target
        sender = f"{request.user.username}@{DOMAIN}"

        # Verify the recipient exists before saving
        if "#" not in target:
            username, domain = receiver.rsplit("@", 1)
            if domain == DOMAIN:
                user = db.session.query(User).filter_by(username=username, is_deleted=False).first()
                if not user:
                    abort(404, message="User not found")
            else:
                try:
                    base = get_endpoint(domain, "userserver", "users")
                    if not base:
                        base = f"http://{domain}/api/users"
                    resp = http_requests.get(f"{base}/{receiver}", timeout=3)
                    if resp.status_code != 200:
                        abort(404, message="User not found on remote server")
                except http_requests.RequestException:
                    abort(502, message="Failed to reach remote server")

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
