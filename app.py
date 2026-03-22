import sys, uuid, requests, os
from flask import Flask, request, jsonify, render_template, session, redirect, url_for, send_file, send_from_directory
from flask_sqlalchemy import SQLAlchemy
from datetime import datetime
from dotenv import load_dotenv
import os, re, markdown, hashlib
from flask import send_from_directory, url_for
from werkzeug.utils import secure_filename
import mimetypes

# Config & Logging
basedir = os.path.abspath(os.path.dirname(__file__))
env_file = sys.argv[1] if len(sys.argv) > 1 else ".env"
load_dotenv(env_file)

PORT = int(os.getenv("PORT", 5000))
DOMAIN = os.getenv("DOMAIN", f"localhost:{PORT}")
DB_NAME = os.getenv("DB_NAME", f"database_{PORT}.db")
SECRET_KEY = os.getenv("SECRET_KEY", "default_secret_key")
CACHE_DIR = os.getenv("CACHE_DIR", "media_cache")
CACHE_TIME = int(os.getenv("CACHE_TIME", 3600)) # in seconden (1 uur)
UPLOAD_FOLDER = os.getenv("UPLOAD_DIR", "uploads")
DB_NAME = os.path.join(basedir, DB_NAME)
UPLOAD_FOLDER = os.path.join(basedir, UPLOAD_FOLDER)

if not os.path.exists(CACHE_DIR):
    os.makedirs(CACHE_DIR)
if not os.path.exists(UPLOAD_FOLDER):
    os.makedirs(UPLOAD_FOLDER)

print(f"--- Node Configuration ---")
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
db = SQLAlchemy(app)

class Message(db.Model):
    # ID is nu: domain/uuid om conflicten te voorkomen
    id = db.Column(db.String(255), primary_key=True) 
    sender = db.Column(db.String(100))
    receiver = db.Column(db.String(100))
    text = db.Column(db.Text)
    validation_key = db.Column(db.String(50))
    timestamp = db.Column(db.DateTime, default=datetime.utcnow)

with app.app_context():
    db.create_all()

# --- API ---

@app.route("/api/chats")
def get_chats():
    if 'username' not in session: return "Unauthorized", 401
    me = session['username']
    # Zoek alle unieke gesprekspartners
    senders = db.session.query(Message.sender).filter(Message.receiver == me).distinct()
    receivers = db.session.query(Message.receiver).filter(Message.sender == me).distinct()
    partners = set([s[0] for s in senders] + [r[0] for r in receivers])
    return jsonify(list(partners))

@app.route("/api/messages/<path:partner>")
def get_messages(partner):
    if 'username' not in session: return "Unauthorized", 401
    me = session['username']
    msgs = Message.query.filter(
        ((Message.sender == me) & (Message.receiver == partner)) |
        ((Message.sender == partner) & (Message.receiver == me))
    ).order_by(Message.timestamp.asc()).all()
    
    return jsonify([{
        "id": m.id, "sender": m.sender, "text": m.text, 
        "time": m.timestamp.strftime("%H:%M")
    } for m in msgs])

@app.route("/api/sendmessage", methods=["POST"])
def send_message():
    if 'username' not in session: return "Unauthorized", 401
    data = request.json
    msg_uuid = str(uuid.uuid4())
    full_id = f"{DOMAIN}/{msg_uuid}" # Unieke ID over federatie heen
    val_key = "key-" + msg_uuid[:8]
    new_msg = Message(id=full_id, sender=session['username'], receiver=data['receiver'], 
                      text=data['messageText'], validation_key=val_key)
    db.session.add(new_msg)
    db.session.commit()
    receiver = data['receiver'] # e.g. "domain.com#general#news"

    # --- CHANNEL LOGIC ---
    if "#" in receiver:
        target_domain = receiver.split('#')[0]
        payload = {"id": full_id, "sender": session['username'], "receiver": receiver, 
                   "text": data['messageText'], "validationKey": val_key}
        try:
            # Assuming channel server runs on a known port or same domain
            requests.post(f"http://{target_domain}/api/channel/send", json=payload, timeout=3)
            return jsonify({"status": "Sent to Channel"})
        except:
            return jsonify({"error": "Channel Server Offline"}), 500
    else:
        

        target_domain = receiver.split('@')[-1]
        payload = {"id": full_id, "sender": session['username'], "receiver": receiver, 
                   "text": data['messageText'], "validationKey": val_key}

        try:
            requests.post(f"http://{target_domain}/federation/receive", json=payload, timeout=3)
            return jsonify({"status": "Sent"})
        except:
            return jsonify({"error": "Offline"}), 500

@app.route("/federation/receive", methods=["POST"])
def receive_message():
    data = request.json
    sender_domain = data['sender'].split('@')[-1]
    val_params = {"messageId": data['id'], "validationKey": data['validationKey']}
    
    try:
        val_resp = requests.get(f"http://{sender_domain}/federation/validate", params=val_params, timeout=3)
        if val_resp.json().get("valid"):
            received = Message(id=data['id'], sender=data['sender'], receiver=data['receiver'], text=data['text'])
            db.session.add(received)
            db.session.commit()
            return "OK", 200
    except: pass
    return "Invalid", 401

@app.route("/federation/validate")
def validate_message():
    msg = Message.query.get(request.args.get("messageId"))
    if msg and msg.validation_key == request.args.get("validationKey"):
        return jsonify({"valid": True})
    return jsonify({"valid": False})

@app.route("/login", methods=["GET", "POST"])
def login():
    if request.method == "POST":
        session['username'] = f"{request.form['user']}@{DOMAIN}"
        return redirect(url_for('index'))
    return '<body style="background:#121212;color:white;display:flex;justify-content:center;align-items:center;height:100vh;font-family:sans-serif;"><form method="post"><h1>Login</h1><input name="user" placeholder="username" required><button>Enter</button></form></body>'

@app.route("/")
def index():
    if 'username' not in session: return redirect(url_for('login'))
    return render_template('index.html', user=session['username'])

#media stuff
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

if __name__ == "__main__":
    app.run(port=PORT)
