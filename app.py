import sys, uuid, requests, os, io, re, markdown, hashlib
from flask import Flask, request, jsonify, render_template, session, redirect, url_for, send_file, send_from_directory
from flask_sqlalchemy import SQLAlchemy
from datetime import datetime
from dotenv import load_dotenv
from werkzeug.utils import secure_filename
import mimetypes
from json_discovery import get_endpoint
from web import web_bp
from federation import federation_bp
import secrets
from werkzeug.security import generate_password_hash, check_password_hash
import pyotp


# Config & Logging
basedir = os.path.abspath(os.path.dirname(__file__))
# 1. Determine if a custom file was provided via command line
custom_env = sys.argv[1] if len(sys.argv) > 1 else None
env_file = custom_env if custom_env else ".env"
env_path = os.path.join(basedir, env_file)

# 2. If the user explicitly provided a file but it doesn't exist, raise an error
if custom_env and not os.path.exists(env_path):
    raise FileNotFoundError(f"Specified env file not found: {env_path}")

# 3. Load the file (load_dotenv returns False if the file isn't found/loaded)

load_dotenv(env_path)

PORT = int(os.getenv("PORT", 5000))
DOMAIN = os.getenv("DOMAIN", f"localhost:{PORT}")
DB_NAME = os.path.join(basedir, os.getenv("DB_NAME", "data/userserver.db"))
SECRET_KEY = os.getenv("SECRET_KEY", "default_secret_key")
CACHE_DIR = os.path.join(basedir, os.getenv("CACHE_DIR", "media_cache"))
CACHE_TIME = int(os.getenv("CACHE_TIME", 3600)) # in seconden (1 uur)
UPLOAD_FOLDER = os.path.join(basedir, os.getenv("UPLOAD_DIR", "uploads"))


# Create directories if they don't exist
os.makedirs(os.path.dirname(DB_NAME), exist_ok=True)
os.makedirs(CACHE_DIR, exist_ok=True)
os.makedirs(UPLOAD_FOLDER, exist_ok=True)

print(f"--- UserNode Configuration ---")
print(f"Configuratie: {env_file}")
print(f"Domain:       {DOMAIN}")
print(f"Port:         {PORT}")
print(f"Database:     {DB_NAME}")
print(f"Cache_Dir:    {CACHE_DIR}")
print(f"Cache_time:   {CACHE_TIME}")
print(f"Upload_Dir:   {UPLOAD_FOLDER}")
print(f"----------------------")

app = Flask(__name__)
app.secret_key = SECRET_KEY
app.config['SQLALCHEMY_DATABASE_URI'] = f'sqlite:///{DB_NAME}'
app.config['SQLALCHEMY_TRACK_MODIFICATIONS'] = False
app.config['UPLOAD_FOLDER'] = UPLOAD_FOLDER
app.config['MAX_CONTENT_LENGTH'] = 16 * 1024 * 1024  # Limiet op 16MB
app.config['DOMAIN'] = DOMAIN
db = SQLAlchemy(app)

# Register blueprints
app.register_blueprint(web_bp)
app.register_blueprint(federation_bp)

class Message(db.Model):
    # ID is nu: domain/uuid om conflicten te voorkomen
    id = db.Column(db.String(255), primary_key=True) 
    sender = db.Column(db.String(100))
    receiver = db.Column(db.String(100))
    text = db.Column(db.Text)
    validation_key = db.Column(db.String(50))
    timestamp = db.Column(db.DateTime, default=datetime.utcnow)

class User(db.Model):
    id = db.Column(db.Integer, primary_key=True)
    username = db.Column(db.String(80), unique=True, nullable=False)
    password_hash = db.Column(db.String(200), nullable=False)
    email = db.Column(db.String(120))
    otp_secret = db.Column(db.String(32), default=pyotp.random_base32)
    is_2fa_enabled = db.Column(db.Boolean, default=False)
    is_admin = db.Column(db.Boolean, default=False)

    # Relatie naar actieve apparaten/sessies
    sessions = db.relationship('UserSession', backref='user', lazy=True, cascade="all, delete-orphan")

class UserSession(db.Model):
    id = db.Column(db.Integer, primary_key=True)
    user_id = db.Column(db.Integer, db.ForeignKey('user.id'), nullable=False)
    token = db.Column(db.String(64), unique=True, nullable=False) # Het unieke apparaat-token
    device_info = db.Column(db.String(255)) # Bijv. "Chrome on Windows"
    last_active = db.Column(db.DateTime, default=datetime.utcnow)
    expires_at = db.Column(db.DateTime, nullable=False)

class InviteCode(db.Model):
    id = db.Column(db.Integer, primary_key=True)
    code = db.Column(db.String(64), unique=True, nullable=False)
    created_by = db.Column(db.Integer, db.ForeignKey('user.id'), nullable=False)
    used_by = db.Column(db.Integer, db.ForeignKey('user.id'))
    created_at = db.Column(db.DateTime, default=datetime.utcnow)
    used_at = db.Column(db.DateTime)
    expires_at = db.Column(db.DateTime)


with app.app_context():
    db.create_all()

# --- API ---

@app.route("/api/chats")
def get_chats():
    if not hasattr(request, 'user') or request.user is None:
        return "Unauthorized", 401
    me = request.user.username
    # Zoek alle unieke gesprekspartners
    senders = db.session.query(Message.sender).filter(Message.receiver == me).distinct()
    receivers = db.session.query(Message.receiver).filter(Message.sender == me).distinct()
    partners = set([s[0] for s in senders] + [r[0] for r in receivers])
    return jsonify(list(partners))

@app.route("/api/messages/<path:target>")
def get_messages(target):
    if not hasattr(request, 'user') or request.user is None:
        return "Unauthorized", 401
    me = request.user.username

    since = request.args.get("since", type=float)    # Get messages AFTER this time
    before = request.args.get("before", type=float)  # Get messages BEFORE this time (for history)
    limit = request.args.get("limit", type=int, default=50)

    # BRANCH: Channel Server (External)
    if "#" in target:
        target_domain = target.split('#')[0]
        channel_url = get_endpoint(target_domain, "channelserver", "channel_poll")
        if not channel_url:
            channel_url = f"http://{target_domain}/api/channel/poll"  # Fallback
        params = {"path": target, "limit": limit, "since": since, "before": before}
        try:
            return jsonify(requests.get(channel_url, params=params).json())
        except:
            return jsonify([]), 500
    
    query = Message.query.filter(
        ((Message.sender == me) & (Message.receiver == target)) |
        ((Message.sender == target) & (Message.receiver == me))
    )

    if since:
        # Convert epoch back to datetime for SQLAlchemy
        query = query.filter(Message.timestamp > datetime.fromtimestamp(since))
    if before:
        query = query.filter(Message.timestamp < datetime.fromtimestamp(before))

    msgs = query.order_by(Message.timestamp.desc()).limit(limit).all()
    
    return jsonify([{
        "id": m.id, 
        "sender": m.sender, 
        "text": m.text, 
        "time": m.timestamp.timestamp() # Sends as Unix Epoch (float)
    } for m in reversed(msgs)])

@app.route("/api/sendmessage", methods=["POST"])
def send_message():
    if not hasattr(request, 'user') or request.user is None:
        return "Unauthorized", 401
    data = request.json
    msg_uuid = str(uuid.uuid4())
    full_id = f"{DOMAIN}/{msg_uuid}" # Unieke ID over federatie heen
    val_key = "key-" + msg_uuid[:8]
    new_msg = Message(id=full_id, sender=request.user.username, receiver=data['receiver'], 
                      text=data['messageText'], validation_key=val_key)
    db.session.add(new_msg)
    db.session.commit()
    receiver = data['receiver'] # e.g. "domain.com#general#news"

    # --- CHANNEL LOGIC ---
    if "#" in receiver:
        target_domain = receiver.split('#')[0]
        channel_url = get_endpoint(target_domain, "channelserver", "channel_send")
        if not channel_url:
            channel_url = f"http://{target_domain}/api/channel/send"  # Fallback
        payload = {"id": full_id, "sender": request.user.username, "receiver": receiver,
                   "text": data['messageText'], "validationKey": val_key}
        try:
            requests.post(channel_url, json=payload, timeout=3)
            return jsonify({"status": "Sent to Channel"})
        except:
            return jsonify({"error": "Channel Server Offline"}), 500
    else:
        

        target_domain = receiver.split('@')[-1]
        payload = {"id": full_id, "sender": request.user.username, "receiver": receiver,
                   "text": data['messageText'], "validationKey": val_key}
        Send_URL = get_endpoint(target_domain, "userserver", "federation_receive")

        if not Send_URL:
            Send_URL = f"http://{target_domain}/federation/receive"  # Fallback

        try:
            requests.post(Send_URL, json=payload, timeout=5)
            return jsonify({"status": "Sent"})
        except Exception as e:
            print(f"Validation error: {e}")            
            return jsonify({"error": "Offline"}), 500

@app.route("/media/proxy")
def media_proxy():
    url = request.args.get("url")
    if not url: return "Missing URL", 400
    
    file_hash = hashlib.md5(url.encode()).hexdigest()
    file_path = os.path.join(CACHE_DIR, file_hash)
    
    # Bepaal het MIME-type op basis van de URL (bijv. image/png)
    mimetype, _ = mimetypes.guess_type(url)
    if not mimetype: mimetype = 'image/jpeg' # Fallback

    # 1. Serveer uit cache indien aanwezig en vers
    if os.path.exists(file_path):
        mtime = os.path.getmtime(file_path)
        if (datetime.utcnow().timestamp() - mtime) < CACHE_TIME:
            return send_file(file_path, mimetype=mimetype)

    # 2. Downloaden als het niet in cache zit
    try:
        r = requests.get(url, timeout=10)
        if r.status_code == 200:
            with open(file_path, 'wb') as f:
                f.write(r.content)
            # Gebruik BytesIO om de zojuist gedownloade content direct te sturen
            return send_file(io.BytesIO(r.content), mimetype=mimetype)
    except Exception as e:
        print(f"Proxy error: {e}")
        return "Failed to fetch image", 500

@app.route("/api/upload", methods=["POST"])
def upload_file():
    if 'file' not in request.files: return "No file", 400
    file = request.files['file']
    if file.filename == '': return "No filename", 400
    
    filename = secure_filename(f"{uuid.uuid4()}_{file.filename}")
    file.save(os.path.join(app.config['UPLOAD_FOLDER'], filename))
    
    # Genereer de Markdown tag voor de gebruiker
    file_url = f"http://{DOMAIN}/uploads/{filename}"
    return jsonify({"markdown": f"![image]({file_url})", "url": file_url})

@app.route("/uploads/<filename>")
def serve_upload(filename):
    return send_from_directory(app.config['UPLOAD_FOLDER'], filename)

@app.route("/.well-known/BSCP/userserver")
def serve_userserver_config():
    """Serve BSCP userserver configuration in JSON format"""
    config = {
        "server": {
            "name": "BSCP User Server",
            "version": "1.0",
            "type": "userserver"
        },
        "api": {
            "base": f"http://{DOMAIN}",
            "endpoints": {
                "chats": "/api/chats",
                "messages": "/api/messages",
                "send_message": "/api/sendmessage",
                "federation_receive": "/federation/receive",
                "federation_validate": "/federation/validate",
                "upload": "/api/upload",
                "media_proxy": "/media/proxy"
            }
        },
        "capabilities": {
            "federation": True,
            "channels": False,
            "direct_messaging": True,
            "media_upload": True
        }
    }
    
    return config, 200, {"Content-Type": "application/json; charset=utf-8"}

if __name__ == "__main__":
    app.run(port=PORT)
