import sys, uuid, requests, os, io, re, markdown, hashlib, json
from flask import Flask, request, jsonify, send_file, send_from_directory
from flask_sqlalchemy import SQLAlchemy
from datetime import datetime
from dotenv import load_dotenv
from werkzeug.utils import secure_filename
import mimetypes
from json_discovery import get_endpoint
from web import web_bp
from federation import federation_bp
import pyotp
import threading
import time


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
CACHE_METADATA_FILE = os.path.join(CACHE_DIR, ".cache_metadata.json")
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

# --- CACHE CLEANUP BACKGROUND THREAD ---
def load_cache_metadata():
    """Load cache metadata (creation times) from file"""
    if os.path.exists(CACHE_METADATA_FILE):
        try:
            with open(CACHE_METADATA_FILE, 'r') as f:
                return json.load(f)
        except:
            return {}
    return {}

def save_cache_metadata(metadata):
    """Save cache metadata to file"""
    try:
        with open(CACHE_METADATA_FILE, 'w') as f:
            json.dump(metadata, f)
    except Exception as e:
        print(f"[CACHE] Failed to save metadata: {e}")

def cleanup_old_cache_files():
    """Periodically remove cache files older than CACHE_TIME"""
    print(f"[CACHE] Cleanup thread started. CACHE_TIME={CACHE_TIME}s, CACHE_DIR={CACHE_DIR}")
    while True:
        try:
            if not os.path.exists(CACHE_DIR):
                continue

            current_time = datetime.utcnow().timestamp()
            metadata = load_cache_metadata()
            deleted_count = 0
            files_in_dir = [f for f in os.listdir(CACHE_DIR) if os.path.isfile(os.path.join(CACHE_DIR, f)) and f != '.cache_metadata.json']

            for filename in files_in_dir:
                file_path = os.path.join(CACHE_DIR, filename)
                creation_time = metadata.get(filename)

                if creation_time is None:
                    # File exists but no metadata - create metadata for it
                    creation_time = current_time
                    metadata[filename] = creation_time
                    print(f"[CACHE] Created metadata for: {filename}")
                else:
                    creation_time = float(creation_time)
                    age = current_time - creation_time
                    should_delete = age > CACHE_TIME
                    print(f"[CACHE] {filename}: age={int(age)}s, limit={CACHE_TIME}s, delete={should_delete}")

                    if should_delete:
                        try:
                            os.remove(file_path)
                            del metadata[filename]
                            deleted_count += 1
                            print(f"[CACHE] ✓ Deleted: {filename}")
                        except OSError as e:
                            print(f"[CACHE] ✗ Failed to delete {filename}: {e}")

            # Clean up metadata for files that no longer exist
            for filename in list(metadata.keys()):
                if not os.path.exists(os.path.join(CACHE_DIR, filename)):
                    del metadata[filename]

            save_cache_metadata(metadata)
            print(f"[CACHE] Scan complete: {len(files_in_dir)} files, {deleted_count} deleted")
        except Exception as e:
            print(f"[CACHE] Cleanup error: {type(e).__name__}: {e}")
        time.sleep(3600)
    

# Start cleanup thread as daemon
cleanup_thread = threading.Thread(target=cleanup_old_cache_files, daemon=True)
cleanup_thread.start()
print("[CACHE] Cleanup thread started")



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
    is_deleted = db.Column(db.Boolean, default=False)

    # User preferences
    display_name = db.Column(db.String(100))
    theme = db.Column(db.String(20), default='dark')
    accent_color = db.Column(db.String(7), default='#7eafff')

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
    my_full_identity = f"{me}@{DOMAIN}"

    # Find all users I've communicated with
    from sqlalchemy import or_

    # Messages where I'm the sender (exact match on my full identity)
    sent_to = db.session.query(Message.receiver).filter(
        Message.sender == my_full_identity
    ).distinct()

    # Messages where I'm the receiver (exact match on my full identity)
    received_from = db.session.query(Message.sender).filter(
        Message.receiver == my_full_identity
    ).distinct()

    partners = set()

    # Add senders of messages I received (always have domain now)
    for sender in received_from:
        sender_name = sender[0]
        partners.add(sender_name)

    # Add receivers of messages I sent (always have domain now)
    for receiver in sent_to:
        receiver_name = receiver[0]
        partners.add(receiver_name)

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

    # Query for direct messages - both sender and receiver always include domain
    from sqlalchemy import or_

    # Build target with domain if not already present
    target_with_domain = f"{target}@{DOMAIN}" if '@' not in target else target

    # print(f"\n[MESSAGES DEBUG] Querying for user '{me}' talking to '{target}'")
    # print(f"[MESSAGES DEBUG] Target normalized to: '{target_with_domain}'")
    # print(f"[MESSAGES DEBUG] Looking for:")
    # print(f"[MESSAGES DEBUG]   - Sent: sender starts with '{me}@' AND receiver == '{target_with_domain}'")
    # print(f"[MESSAGES DEBUG]   - Received: sender == '{target_with_domain}' AND receiver starts with '{me}@'")

    # First, check what's actually in the database
    # all_messages = db.session.query(Message).all()
    # print(f"[MESSAGES DEBUG] Total messages in database: {len(all_messages)}")
    # for m in all_messages:
    #     print(f"[MESSAGES DEBUG]   - ID: {m.id}, Sender: {m.sender}, Receiver: {m.receiver}")

    # Build all possible variations of the target name for matching
    # ONLY match on domain-qualified names to prevent cross-instance message leaks
    target_variations = [target_with_domain]  # e.g., ["bob@localhost:5000"]

    # Build my full user@domain identifier for exact matching
    my_full_identity = f"{me}@{DOMAIN}"

    # Messages where I sent to target
    sent_condition = (Message.sender == my_full_identity) & (Message.receiver == target_with_domain)

    # Messages where target sent to me
    received_condition = (Message.sender == target_with_domain) & (Message.receiver == my_full_identity)

    query = Message.query.filter(or_(sent_condition, received_condition))

    if since:
        query = query.filter(Message.timestamp > datetime.fromtimestamp(since))
    if before:
        query = query.filter(Message.timestamp < datetime.fromtimestamp(before))

    msgs = query.order_by(Message.timestamp.desc()).limit(limit).all()

    #print(f"[MESSAGES] Query for user '{me}' talking to '{target}' (normalized to '{target_with_domain}')")
    #print(f"[MESSAGES] Found {len(msgs)} messages")
    #for m in msgs:
        #print(f"  - {m.sender} -> {m.receiver}: {m.text[:50] if m.text else 'NO TEXT'}")

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

    # Normalize receiver to always include domain
    receiver = data['receiver']
    if '@' not in receiver:
        # Local message - add our domain
        receiver_normalized = f"{receiver}@{DOMAIN}"
    else:
        # Remote message - already has domain
        receiver_normalized = receiver

    new_msg = Message(id=full_id, sender=f"{request.user.username}@{DOMAIN}", receiver=receiver_normalized,
                      text=data['messageText'], validation_key=val_key)
    db.session.add(new_msg)
    db.session.commit()

    # --- CHANNEL LOGIC ---
    if "#" in receiver:
        target_domain = receiver.split('#')[0]
        channel_url = get_endpoint(target_domain, "channelserver", "channel_send")
        if not channel_url:
            channel_url = f"http://{target_domain}/api/channel/send"  # Fallback
        payload = {"id": full_id, "sender": f"{request.user.username}@{DOMAIN}", "receiver": receiver,
                   "text": data['messageText'], "validationKey": val_key}
        try:
            requests.post(channel_url, json=payload, timeout=3)
            return jsonify({"status": "Sent to Channel"})
        except:
            return jsonify({"error": "Channel Server Offline"}), 500
    else:
        # Direct message
        if '@' not in receiver:
            # Local message - just username
            target_domain = DOMAIN
        else:
            target_domain = receiver.split('@')[-1]
        payload = {"id": full_id, "sender": f"{request.user.username}@{DOMAIN}", "receiver": receiver_normalized,
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
    if not hasattr(request, 'user') or request.user is None:
        return "Unauthorized", 401
    url = request.args.get("url")
    if not url: return "Missing URL", 400
    file_hash = hashlib.md5(url.encode()).hexdigest()
    file_path = os.path.join(CACHE_DIR, file_hash)
    # Bepaal het MIME-type op basis van de URL (bijv. image/png)
    mimetype, _ = mimetypes.guess_type(url)
    if not mimetype: mimetype = 'image/jpeg' # Fallback

    # 1. Serveer uit cache indien aanwezig en vers
    if os.path.exists(file_path):
        metadata = load_cache_metadata()
        creation_time = metadata.get(file_hash)
        if creation_time:
            creation_time = float(creation_time)
            age = datetime.utcnow().timestamp() - creation_time
            if age < CACHE_TIME:
                return send_file(file_path, mimetype=mimetype)

    # 2. Downloaden als het niet in cache zit
    try:
        r = requests.get(url, timeout=10)
        if r.status_code == 200:
            with open(file_path, 'wb') as f:
                f.write(r.content)
            # Update metadata with creation time
            metadata = load_cache_metadata()
            metadata[file_hash] = datetime.utcnow().timestamp()
            save_cache_metadata(metadata)
            # Gebruik BytesIO om de zojuist gedownloade content direct te sturen
            return send_file(io.BytesIO(r.content), mimetype=mimetype)
    except Exception as e:
        print(f"Proxy error: {e}")
        return "Failed to fetch image", 500

@app.route("/api/upload", methods=["POST"])
def upload_file():
    if not hasattr(request, 'user') or request.user is None:
        return "Unauthorized", 401
    if 'file' not in request.files: return "No file", 400
    file = request.files['file']
    if file.filename == '': return "No filename", 400

    filename = secure_filename(f"{uuid.uuid4()}_{file.filename}")
    file.save(os.path.join(app.config['UPLOAD_FOLDER'], filename))

    # Genereer de Markdown tag voor de gebruiker
    file_url = f"http://{DOMAIN}/uploads/{filename}"
    return jsonify({"markdown": f"![image]({file_url})", "url": file_url})

@app.route("/api/settings", methods=["GET"])
def get_settings():
    if not hasattr(request, 'user') or request.user is None:
        return "Unauthorized", 401

    user = request.user
    return jsonify({
        "display_name": user.display_name or user.username,
        "theme": user.theme or "dark",
        "accent_color": user.accent_color or "#7eafff"
    })

@app.route("/api/settings", methods=["POST"])
def update_settings():
    if not hasattr(request, 'user') or request.user is None:
        return "Unauthorized", 401

    data = request.get_json()
    user = request.user

    if 'display_name' in data:
        user.display_name = data['display_name']
    if 'theme' in data:
        user.theme = data['theme']
    if 'accent_color' in data:
        user.accent_color = data['accent_color']

    db.session.commit()
    return jsonify({"success": True})

@app.route("/uploads/<filename>")
def serve_upload(filename):
    return send_from_directory(app.config['UPLOAD_FOLDER'], filename)

@app.route("/.well-known/BSCP/userserver")
@app.route("/.well-known/BSCP/userserver.json")
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
