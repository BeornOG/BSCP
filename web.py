"""Web UI routes for BSCP"""
from flask import Blueprint, request, render_template, session, redirect, url_for, send_from_directory
from flask import current_app as app

web_bp = Blueprint('web', __name__)


@web_bp.route("/login", methods=["GET", "POST"])
def login():
    if request.method == "POST":
        session['username'] = f"{request.form['user']}@{app.config.get('DOMAIN', 'localhost')}"
        return redirect(url_for('web.index'))
    return '<body style="background:#121212;color:white;display:flex;justify-content:center;align-items:center;height:100vh;font-family:sans-serif;"><form method="post"><h1>Login</h1><input name="user" placeholder="username" required><button>Enter</button></form></body>'


@web_bp.route("/")
def index():
    if 'username' not in session:
        return redirect(url_for('web.login'))
    return render_template('index.html', user=session['username'])
