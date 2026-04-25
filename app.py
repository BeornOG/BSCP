import sys, uuid, requests, os, io, hashlib, json
from flask import Flask, request, jsonify, send_file, send_from_directory
from flask_sqlalchemy import SQLAlchemy
from flask_smorest import Api
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
custom_env = None
custom_db = None

# Parse command line arguments
for i, arg in enumerate(sys.argv[1:], 1):
    if arg.startswith("--db="):
        custom_db = arg.split("=", 1)[1]
    elif arg.startswith("--db"):
        # --db followed by space-separated path
        if i < len(sys.argv) - 1:
            custom_db = sys.argv[i + 1]
    elif not arg.startswith("--") and custom_env is None:
        # First non-flag argument is the env file
        custom_env = arg

env_file = custom_env if custom_env else ".env"
env_path = os.path.join(basedir, env_file)

# 2. If the user explicitly provided a file but it doesn't exist, raise an error
if custom_env and not os.path.exists(env_path):
    raise FileNotFoundError(f"Specified env file not found: {env_path}")

# 3. Load the file (load_dotenv returns False if the file isn't found/loaded)
load_dotenv(env_path)

PORT = int(os.getenv("PORT", 5000))
DOMAIN = os.getenv("DOMAIN", f"localhost:{PORT}")

# 4. Handle database file - custom arg overrides env var
if custom_db:
    # If custom_db is not absolute, make it relative to basedir
    DB_NAME = custom_db if os.path.isabs(custom_db) else os.path.join(basedir, custom_db)
else:
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
print(f"Database:     {DB_NAME}" + (f" (custom)" if custom_db else ""))
print(f"Cache_Dir:    {CACHE_DIR}")
print(f"Cache_time:   {CACHE_TIME}")
print(f"Upload_Dir:   {UPLOAD_FOLDER}")
print(f"----------------------")

STATIC_DIR = os.path.join(basedir, "static")

app = Flask(__name__, static_folder=STATIC_DIR, static_url_path='/static')
app.secret_key = SECRET_KEY
app.config['SQLALCHEMY_DATABASE_URI'] = f'sqlite:///{DB_NAME}'
app.config['SQLALCHEMY_TRACK_MODIFICATIONS'] = False
app.config['UPLOAD_FOLDER'] = UPLOAD_FOLDER
app.config['MAX_CONTENT_LENGTH'] = 16 * 1024 * 1024  # Limiet op 16MB
app.config['DOMAIN'] = DOMAIN
app.config['VAPID_PUBLIC_KEY'] = os.getenv('VAPID_PUBLIC_KEY', '').strip()
app.config['VAPID_PRIVATE_KEY'] = os.getenv('VAPID_PRIVATE_KEY', '').strip()
app.config['VAPID_CONTACT'] = os.getenv('VAPID_CONTACT', 'mailto:admin@localhost')

_VAPID_KEYS_FILE = os.path.join(basedir, 'vapid_keys.json')

if not app.config['VAPID_PRIVATE_KEY'] or not app.config['VAPID_PUBLIC_KEY']:
    if os.path.exists(_VAPID_KEYS_FILE):
        try:
            with open(_VAPID_KEYS_FILE, 'r') as _f:
                _keys = json.load(_f)
            app.config['VAPID_PRIVATE_KEY'] = _keys['private_key']
            app.config['VAPID_PUBLIC_KEY'] = _keys['public_key']
            print('[VAPID] Loaded persistent VAPID keys.')
        except Exception as exc:
            print(f"[VAPID] Failed to load VAPID keys from file: {exc}")

if not app.config['VAPID_PRIVATE_KEY'] or not app.config['VAPID_PUBLIC_KEY']:
    try:
        from cryptography.hazmat.primitives.asymmetric import ec
        from cryptography.hazmat.primitives import serialization
        from base64 import urlsafe_b64encode

        private_key = ec.generate_private_key(ec.SECP256R1())
        private_bytes = private_key.private_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PrivateFormat.PKCS8,
            encryption_algorithm=serialization.NoEncryption(),
        )
        public_bytes = private_key.public_key().public_bytes(
            encoding=serialization.Encoding.X962,
            format=serialization.PublicFormat.UncompressedPoint,
        )
        app.config['VAPID_PRIVATE_KEY'] = private_bytes.decode('utf-8')
        app.config['VAPID_PUBLIC_KEY'] = urlsafe_b64encode(public_bytes).decode('ascii').rstrip('=')
        try:
            with open(_VAPID_KEYS_FILE, 'w') as _f:
                json.dump({'private_key': app.config['VAPID_PRIVATE_KEY'],
                           'public_key': app.config['VAPID_PUBLIC_KEY']}, _f)
            print('[VAPID] Generated and saved persistent VAPID keys.')
        except Exception as exc:
            print(f"[VAPID] Could not save VAPID keys to file: {exc}")
    except Exception as exc:
        print(f"[VAPID] Failed to generate VAPID keys: {exc}")

# OpenAPI / flask-smorest configuration
app.config['API_TITLE'] = 'BSCP API'
app.config['API_VERSION'] = 'v1'
app.config['OPENAPI_VERSION'] = '3.0.3'
app.config['OPENAPI_URL_PREFIX'] = '/api/docs'
app.config['OPENAPI_JSON_PATH'] = 'openapi.json'
app.config['OPENAPI_SWAGGER_UI_PATH'] = '/'
app.config['OPENAPI_SWAGGER_UI_URL'] = 'https://cdn.jsdelivr.net/npm/swagger-ui-dist/'
app.config['ETAG_DISABLED'] = True

db = SQLAlchemy(app)
api = Api(app)

# Register standard Flask blueprints
app.register_blueprint(web_bp)
app.register_blueprint(federation_bp)

# Register flask-smorest API blueprints
from routes import ALL_BLUEPRINTS
for blp in ALL_BLUEPRINTS:
    api.register_blueprint(blp)

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
    timestamp = db.Column(db.DateTime, default=datetime.now)
    is_read = db.Column(db.Boolean, default=False)

class User(db.Model):
    id = db.Column(db.String(255), primary_key=True, default=lambda: str(uuid.uuid4()))
    username = db.Column(db.String(80), unique=True, nullable=False)
    password_hash = db.Column(db.String(200), nullable=False)
    email = db.Column(db.String(120))
    otp_secret = db.Column(db.String(32), default=pyotp.random_base32)
    is_2fa_enabled = db.Column(db.Boolean, default=False)
    is_admin = db.Column(db.Boolean, default=False)
    is_primary_admin = db.Column(db.Boolean, default=False)
    is_deleted = db.Column(db.Boolean, default=False)

    # User preferences
    display_name = db.Column(db.String(100))
    theme = db.Column(db.String(20), default='dark')
    accent_color = db.Column(db.String(7), default='#7eafff')
    bio = db.Column(db.Text)
    profile_pic = db.Column(db.Text)
    Status_Text = db.Column(db.String(32))
    created_at = db.Column(db.DateTime, default=datetime.now)
    Status_type = db.Column(db.Integer)

    # Relatie naar actieve apparaten/sessies
    sessions = db.relationship('UserSession', backref='user', lazy=True, cascade="all, delete-orphan")
    push_subscriptions = db.relationship('PushSubscription', backref='user', lazy=True, cascade="all, delete-orphan")
    webhooks = db.relationship('Webhook', backref='user', lazy=True, cascade="all, delete-orphan")

class UserSession(db.Model):
    id = db.Column(db.String(255), primary_key=True, default=lambda: str(uuid.uuid4()))
    user_id = db.Column(db.String(255), db.ForeignKey('user.id'), nullable=False)
    token = db.Column(db.String(64), unique=True, nullable=False) # Het unieke apparaat-token
    device_info = db.Column(db.String(255)) # Bijv. "Chrome on Windows"
    last_active = db.Column(db.DateTime, default=datetime.now)
    expires_at = db.Column(db.DateTime, nullable=False)

class PushSubscription(db.Model):
    id = db.Column(db.String(255), primary_key=True, default=lambda: str(uuid.uuid4()))
    user_id = db.Column(db.String(255), db.ForeignKey('user.id'), nullable=False)
    endpoint = db.Column(db.Text, nullable=False, unique=True)
    p256dh = db.Column(db.String(255), nullable=False)
    auth = db.Column(db.String(255), nullable=False)
    created_at = db.Column(db.DateTime, default=datetime.now)
    updated_at = db.Column(db.DateTime, default=datetime.now, onupdate=datetime.now)

class Upload(db.Model):
    id = db.Column(db.String(255), primary_key=True, default=lambda: str(uuid.uuid4()))
    filename = db.Column(db.String(255), nullable=False)
    mimetype = db.Column(db.String(100), nullable=False)
    size_bytes = db.Column(db.Integer, nullable=False, default=0)
    uploaded_by = db.Column(db.String(255), db.ForeignKey('user.id'), nullable=False)
    created_at = db.Column(db.DateTime, default=datetime.now)

class ServerConfig(db.Model):
    id = db.Column(db.Integer, primary_key=True, default=1)
    storage_limit_mb = db.Column(db.Integer, default=500)
    updated_at = db.Column(db.DateTime, default=datetime.now, onupdate=datetime.now)

class InviteCode(db.Model):
    id = db.Column(db.Integer, primary_key=True)
    code = db.Column(db.String(64), unique=True, nullable=False)
    created_by = db.Column(db.Integer, db.ForeignKey('user.id'), nullable=False)
    used_by = db.Column(db.Integer, db.ForeignKey('user.id'))
    created_at = db.Column(db.DateTime, default=datetime.now)
    used_at = db.Column(db.DateTime)
    expires_at = db.Column(db.DateTime)

class Webhook(db.Model):
    id = db.Column(db.String(255), primary_key=True, default=lambda: str(uuid.uuid4()))
    user_id = db.Column(db.String(255), db.ForeignKey('user.id'), nullable=False)
    channel_id = db.Column(db.String(255))
    name = db.Column(db.String(100), nullable=False)
    token = db.Column(db.String(64), unique=True, nullable=False)
    profile_pic = db.Column(db.Text)
    created_at = db.Column(db.DateTime, default=datetime.now)
    last_used = db.Column(db.DateTime)


def send_push_notification(user, title, body, url='/'):
    if not user:
        return

    vapid_private_key = app.config.get('VAPID_PRIVATE_KEY')
    vapid_public_key = app.config.get('VAPID_PUBLIC_KEY')
    vapid_contact = app.config.get('VAPID_CONTACT')
    if not vapid_private_key or not vapid_public_key:
        print('[PUSH] VAPID keys are not configured, push cannot be sent.')
        return

    try:
        from pywebpush import webpush, WebPushException
    except ImportError:
        print('[PUSH] pywebpush is not installed; install pywebpush to enable server push notifications.')
        return

    subscriptions = getattr(user, 'push_subscriptions', []) or []
    if not subscriptions:
        return

    payload = json.dumps({
        'title': title,
        'body': body,
        'url': url,
    })

    for subscription in subscriptions:
        info = {
            'endpoint': subscription.endpoint,
            'keys': {
                'p256dh': subscription.p256dh,
                'auth': subscription.auth,
            },
        }
        try:
            webpush(
                subscription_info=info,
                data=payload,
                vapid_private_key=vapid_private_key,
                vapid_claims={'sub': vapid_contact},
            )
        except WebPushException as exc:
            print(f'[PUSH] Failed sending push to {user.username}: {exc}')
            response = getattr(exc, 'response', None)
            if response is not None and response.status_code in (404, 410):
                db.session.delete(subscription)
                db.session.commit()
        except Exception as exc:
            print(f'[PUSH] Unexpected push error for {user.username}: {exc}')


def get_local_user(full_id):
    if '@' not in full_id:
        return None
    username, domain = full_id.rsplit('@', 1)
    if domain != DOMAIN:
        return None
    return db.session.query(User).filter_by(username=username).first()


with app.app_context():
    db.create_all()

# --- NON-API ROUTES (media proxy, uploads, well-known, SPA) ---

@app.route("/media/proxy")
def media_proxy():
    url = request.args.get("url")
    if not url: return "Missing URL", 400
    file_hash = hashlib.md5(url.encode()).hexdigest()
    file_path = os.path.join(CACHE_DIR, file_hash)
    mimetype, _ = mimetypes.guess_type(url)
    if not mimetype: mimetype = 'image/jpeg'

    if os.path.exists(file_path):
        metadata = load_cache_metadata()
        creation_time = metadata.get(file_hash)
        if creation_time:
            age = datetime.utcnow().timestamp() - float(creation_time)
            if age < CACHE_TIME:
                return send_file(file_path, mimetype=mimetype)

    try:
        r = requests.get(url, timeout=10)
        if r.status_code == 200:
            with open(file_path, 'wb') as f:
                f.write(r.content)
            metadata = load_cache_metadata()
            metadata[file_hash] = datetime.utcnow().timestamp()
            save_cache_metadata(metadata)
            return send_file(io.BytesIO(r.content), mimetype=mimetype)
    except Exception as e:
        print(f"Proxy error: {e}")
        return "Failed to fetch image", 500

@app.route("/uploads/<filename>")
def serve_upload(filename):
    return send_from_directory(app.config['UPLOAD_FOLDER'], filename)

@app.route("/webhooks/<webhook_id>/<webhook_token>", methods=["POST"])
def receive_webhook(webhook_id, webhook_token):
    """Receive incoming webhook and send as DM"""
    from schemas import WebhookPayload
    from marshmallow import ValidationError

    payload_schema = WebhookPayload()

    # Parse and validate JSON
    try:
        data = request.get_json()
        if not data:
            return jsonify({"error": "Missing JSON body"}), 400
        data = payload_schema.load(data)
    except ValidationError as err:
        return jsonify({"errors": err.messages}), 400
    except Exception as err:
        return jsonify({"error": str(err)}), 400

    # Find webhook
    webhook = db.session.query(Webhook).filter_by(id=webhook_id, token=webhook_token).first()
    if not webhook:
        return jsonify({"error": "Invalid webhook"}), 404

    # Update last_used
    webhook.last_used = datetime.now()
    db.session.commit()

    # Create message
    msg_uuid = str(uuid.uuid4())
    full_id = f"{DOMAIN}/{msg_uuid}"
    val_key = "key-" + msg_uuid[:8]

    sender = f"webhook-{webhook.id}@{DOMAIN}"

    receiver = f"{webhook.user.username}@{DOMAIN}"

    new_msg = Message(
        id=full_id,
        sender=sender,
        receiver=receiver,
        text=data["content"],
        validation_key=val_key,
    )
    db.session.add(new_msg)
    db.session.commit()

    # Send push notification
    send_push_notification(
        webhook.user,
        f"Message from {webhook.name}",
        data["content"],
        url='/',
    )

    return jsonify({
        "success": True,
        "message_id": full_id,
    }), 201

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
            "docs": "/api/docs/",
            "openapi": "/api/docs/openapi.json",
            "endpoints": {
                "chats": "/api/chats/",
                "messages": "/api/messages/",
                "send_message": "/api/messages/",
                "users_me": "/api/users/me",
                "users": "/api/users/",
                "invites": "/api/invites/",
                "upload": "/api/upload/",
                "auth_login": "/api/auth/login",
                "auth_register": "/api/auth/register",
                "auth_setup": "/api/auth/setup",
                "webhooks": "/api/user/webhooks",
                "federation_receive": "/federation/receive",
                "federation_validate": "/federation/validate",
                "media_proxy": "/media/proxy",
            }
        },
        "capabilities": {
            "federation": True,
            "channels": False,
            "direct_messaging": True,
            "media_upload": True,
            "webhooks": True
        }
    }
    return config, 200, {"Content-Type": "application/json; charset=utf-8"}

# --- SERVE VITE SPA FROM /static ---

@app.route('/')
@app.route('/<path:path>')
def serve_spa(path=''):
    """Serve the Vite SPA. Static assets are served by Flask's static handler,
    all other routes fall through to index.html for client-side routing."""
    # If the path matches an actual file in the static dir, serve it
    file_path = os.path.join(STATIC_DIR, path)
    if path and os.path.isfile(file_path):
        return send_from_directory(STATIC_DIR, path)
    # Otherwise serve index.html for SPA client-side routing
    return send_from_directory(STATIC_DIR, 'index.html')

if __name__ == "__main__":
    app.run(port=PORT, debug=True, use_reloader=True)
