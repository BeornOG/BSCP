import os, sys, requests, os, uuid, secrets
from flask import Flask, request, jsonify
from flask_sqlalchemy import SQLAlchemy
from datetime import datetime
from dotenv import load_dotenv
from json_discovery import get_endpoint

# --- Config ---
basedir = os.path.abspath(os.path.dirname(__file__))
env_file = sys.argv[1] if len(sys.argv) > 1 else ".env"
load_dotenv(env_file)
PORT = int(os.getenv("CH_PORT", 6000))
DB_NAME = os.path.join(basedir, os.getenv("CH_DB_NAME", "data/channelserver.db"))
DOMAIN = os.getenv("DOMAIN", f"localhost:{PORT}")

# Create data directory if it doesn't exist
os.makedirs(os.path.dirname(DB_NAME), exist_ok=True)

app = Flask(__name__)
app.config['SQLALCHEMY_DATABASE_URI'] = f'sqlite:///{DB_NAME}'
app.config['SQLALCHEMY_TRACK_MODIFICATIONS'] = False
db = SQLAlchemy(app)

class ChannelMessage(db.Model):
    id = db.Column(db.String(255), primary_key=True)
    channel_path = db.Column(db.String(255), index=True) # domain#channel#sub
    sender = db.Column(db.String(100))
    text = db.Column(db.Text)
    timestamp = db.Column(db.DateTime, default=datetime.utcnow, index=True)

class ChannelWebhook(db.Model):
    id = db.Column(db.String(255), primary_key=True, default=lambda: str(uuid.uuid4()))
    channel_path = db.Column(db.String(255), nullable=False)
    name = db.Column(db.String(100), nullable=False)
    token = db.Column(db.String(64), unique=True, nullable=False)
    profile_pic = db.Column(db.Text)
    created_at = db.Column(db.DateTime, default=datetime.utcnow)
    last_used = db.Column(db.DateTime)

with app.app_context():
    db.create_all()

@app.route("/api/channel/send", methods=["POST"])
def receive_from_user_server():
    data = request.json
    sender_domain = data['sender'].split('@')[-1]
    val_params = {"messageId": data['id'], "validationKey": data['validationKey']}

    val_url = get_endpoint(sender_domain, "userserver", "federation_validate")
    if not val_url:
        val_url = f"http://{sender_domain}/federation/validate"  # Fallback

    try:
        val_resp = requests.get(val_url, params=val_params, timeout=3)
        if val_resp.json().get("valid"):
            full_id = f"{DOMAIN}/message/{data['id']}"
            new_msg = ChannelMessage(
                id=full_id,
                channel_path=data['receiver'],
                sender=data['sender'],
                text=data['text']
            )
            db.session.add(new_msg)
            db.session.commit()
            return jsonify({"status": "stored", "id": new_msg.id})
    except:
        pass
    return "Invalid", 401
    

@app.route("/api/channel/poll")
def poll_messages():
    path = request.args.get("path")
    limit = request.args.get("limit", type=int, default=50)
    since = request.args.get("since", type=float)   # UTC epoch
    before = request.args.get("before", type=float) # Voor historiek opvragen

    query = ChannelMessage.query.filter(ChannelMessage.channel_path == path)

    if since:
        query = query.filter(ChannelMessage.timestamp > datetime.fromtimestamp(since))
    
    if before:
        query = query.filter(ChannelMessage.timestamp < datetime.fromtimestamp(before))

    # We sorteren op DESC om de nieuwste 'X' berichten te pakken
    msgs = query.order_by(ChannelMessage.timestamp.desc()).limit(limit).all()
    
    # Keer de lijst om voor chronologische weergave in de UI
    return jsonify([{
        "id": m.id,
        "sender": m.sender,
        "text": m.text,
        "time": m.timestamp.timestamp()
    } for m in reversed(msgs)])

@app.route("/api/channel/webhooks", methods=["GET"])
def list_channel_webhooks():
    """List webhooks for a channel"""
    channel_path = request.args.get("path")
    if not channel_path:
        return jsonify({"error": "Missing channel path"}), 400

    webhooks = ChannelWebhook.query.filter_by(channel_path=channel_path).all()
    return jsonify([{
        "id": w.id,
        "name": w.name,
        "url": f"http://{DOMAIN}/webhooks/{w.id}/{w.token}",
        "profile_pic": w.profile_pic,
        "created_at": w.created_at.timestamp(),
        "last_used": w.last_used.timestamp() if w.last_used else None,
    } for w in webhooks])


@app.route("/api/channel/webhooks", methods=["POST"])
def create_channel_webhook():
    """Create a webhook for a channel. Name is immutable and used as message sender identity."""
    data = request.json
    channel_path = data.get("path")
    name = data.get("name")

    if not channel_path or not name:
        return jsonify({"error": "Missing channel path or name"}), 400

    webhook = ChannelWebhook(
        channel_path=channel_path,
        name=name,
        token=secrets.token_urlsafe(32),
        profile_pic=data.get("avatar_url"),
    )
    db.session.add(webhook)
    db.session.commit()

    return jsonify({
        "id": webhook.id,
        "name": webhook.name,
        "url": f"http://{DOMAIN}/webhooks/{webhook.id}/{webhook.token}",
        "profile_pic": webhook.profile_pic,
        "created_at": webhook.created_at.timestamp(),
    }), 201


@app.route("/api/channel/webhooks/<webhook_id>", methods=["DELETE"])
def delete_channel_webhook(webhook_id):
    """Delete a channel webhook"""
    webhook = ChannelWebhook.query.filter_by(id=webhook_id).first()
    if not webhook:
        return jsonify({"error": "Webhook not found"}), 404

    db.session.delete(webhook)
    db.session.commit()
    return "", 204


@app.route("/api/channel/webhooks/<webhook_id>/regenerate", methods=["POST"])
def regenerate_channel_webhook(webhook_id):
    """Regenerate a channel webhook token"""
    webhook = ChannelWebhook.query.filter_by(id=webhook_id).first()
    if not webhook:
        return jsonify({"error": "Webhook not found"}), 404

    webhook.token = secrets.token_urlsafe(32)
    db.session.commit()

    return jsonify({"url": f"http://{DOMAIN}/webhooks/{webhook.id}/{webhook.token}"}), 200


@app.route("/webhooks/<webhook_id>/<webhook_token>", methods=["POST"])
def receive_channel_webhook(webhook_id, webhook_token):
    """Receive incoming webhook and send to channel"""
    data = request.json
    if not data or "content" not in data:
        return jsonify({"error": "Missing content"}), 400

    webhook = ChannelWebhook.query.filter_by(id=webhook_id, token=webhook_token).first()
    if not webhook:
        return jsonify({"error": "Invalid webhook"}), 404

    webhook.last_used = datetime.utcnow()
    db.session.commit()

    msg_uuid = str(uuid.uuid4())
    full_id = f"{DOMAIN}/message/{msg_uuid}"

    sender = f"webhook-{webhook.id}@{DOMAIN}"

    new_msg = ChannelMessage(
        id=full_id,
        channel_path=webhook.channel_path,
        sender=sender,
        text=data["content"],
    )
    db.session.add(new_msg)
    db.session.commit()

    return jsonify({
        "success": True,
        "message_id": full_id,
    }), 201


@app.route("/.well-known/BSCP/channelserver")
def serve_channelserver_config():
    """Serve BSCP channelserver configuration in JSON format"""
    config = {
        "server": {
            "name": "BSCP Channel Server",
            "version": "1.0",
            "type": "channelserver"
        },
        "api": {
            "base": f"http://{DOMAIN}",
            "endpoints": {
                "channel_send": "/api/channel/send",
                "channel_poll": "/api/channel/poll",
                "channel_webhooks": "/api/channel/webhooks"
            }
        },
        "capabilities": {
            "federation": True,
            "channels": True,
            "direct_messaging": False,
            "media_upload": False,
            "webhooks": True
        }
    }
    
    return config, 200, {"Content-Type": "application/json; charset=utf-8"}

if __name__ == "__main__":
    print(f"Channel Server running on port {PORT}")
    app.run(port=PORT)
