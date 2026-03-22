import os, sys, uuid
from flask import Flask, request, jsonify
from flask_sqlalchemy import SQLAlchemy
from datetime import datetime
from dotenv import load_dotenv

# --- Config ---
basedir = os.path.abspath(os.path.dirname(__file__))
env_file = sys.argv[1] if len(sys.argv) > 1 else ".env"
load_dotenv(env_file)
PORT = int(os.getenv("CH_PORT", 6000))
DB_NAME = os.getenv("DB_NAME", f"database_{PORT}.db")
DOMAIN = os.getenv("DOMAIN", f"localhost:{PORT}")
DB_NAME = os.path.join(basedir, DB_NAME)

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
    
    try:
        val_resp = requests.get(f"http://{sender_domain}/federation/validate", params=val_params, timeout=3)
        if val_resp.json().get("valid"):
            full_id = f"{DOMAIN}/message/{data['id']}"
            new_msg = ChannelMessage(
                id = full_id,
            channel_path=data['receiver'],
            sender=data['sender'],
            text=data['text']
        )
        db.session.add(new_msg)
        db.session.commit()
        return jsonify({"status": "stored", "id": new_msg.id})
    except: pass
    return "Invalid", 401
    

@app.route("/api/channel/poll")
def poll_messages():
    path = request.args.get("path")
    limit = request.args.get("limit", type=int, default=50)
    since = request.args.get("since")   # ISO Formaat: 2023-10-01T12:00:00
    before = request.args.get("before") # Voor historiek opvragen

    query = ChannelMessage.query.filter(ChannelMessage.channel_path == path)

    if since:
        dt_since = datetime.fromisoformat(since)
        query = query.filter(ChannelMessage.timestamp > dt_since)
    
    if before:
        dt_before = datetime.fromisoformat(before)
        query = query.filter(ChannelMessage.timestamp < dt_before)

    # We sorteren op DESC om de nieuwste 'X' berichten te pakken
    msgs = query.order_by(ChannelMessage.timestamp.desc()).limit(limit).all()
    
    # Keer de lijst om voor chronologische weergave in de UI
    return jsonify([{
        "id": m.id, 
        "sender": m.sender, 
        "text": m.text, 
        "time": m.timestamp.isoformat()
    } for m in reversed(msgs)])

if __name__ == "__main__":
    print(f"Channel Server running on port {PORT}")
    app.run(port=PORT)
