import os, sys, uuid, requests, os
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
                "channel_poll": "/api/channel/poll"
            }
        },
        "capabilities": {
            "federation": True,
            "channels": True,
            "direct_messaging": False,
            "media_upload": False
        }
    }
    
    return config, 200, {"Content-Type": "application/json; charset=utf-8"}

if __name__ == "__main__":
    print(f"Channel Server running on port {PORT}")
    app.run(port=PORT)
