from waitress import serve
from app import app
import sys
from dotenv import load_dotenv
import os


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

# De logica in app.py zorgt al dat de .env geladen wordt via sys.argv
if __name__ == "__main__":
    print(f"Starting production server with config: {sys.argv[1]}")
    serve(app, host='0.0.0.0', port=PORT)
