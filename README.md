# BSCP
*Beorn's Stupid Chat Protocol* is designed as a federated discord/group chat alternative. It has been designed so that the original sender is responsable for making media available via embedding urls. it also is configured that any recieved media is proxied, and cached by the recuevers user server to prevent media being used to get other uses ip's.

the goal it to also have channel servers. these are intended as replacements for discord guilds. these are intended to store messages in channels and providing the ability for only some people to get acces to certain channels.

a username is defined as ``[user]@[domain]``
a channel is defined as ``[domain]#[channel]#[subchannel]#[subchannel]``

the protocol also supports /.well-known/BSCP/ for usecases where you want to use a domain but don't want the federation api there.

## Prerequisites

- **Python 3.10+**
- **Node.js 18+**
node is only needed to compile front-end, not needed if you pre-compile the front-end on another device

# userserver

## Frontend Setup

1. Install dependencies:

   ```bash
   cd frontend
   npm install
   ```

2. Start the dev server:

   ```bash
   npm exec vite build
   ```


## Backend Setup

1. Install Python dependencies:

   ```bash
   pip install flask flask-sqlalchemy python-dotenv werkzeug pyotp requests qrcode
   ```

2. Create a `.env` file in the project root (or use one of the test configs in `Testing/`):

   ```env
   PORT=5000
   DOMAIN=localhost:5000
   DB_NAME=database.db
   CACHE_DIR=media_cache
   CACHE_TIME=86400
   UPLOAD_DIR=uploads
   ```

3. Start the backend:

   ```bash
   python app.py
   ```

   Or with a specific env file:

   ```bash
   python app.py Testing/NodeA.env
   ```

   The backend runs on `http://localhost:5000` with auto-reloading enabled.



## First-Time Setup

1. Open `http://localhost:5000` in your browser.
2. You'll be redirected to the setup page to create an admin account.
3. After setup, log in with your new credentials.

## Project Structure

```
BSCP/
├── app.py              # Flask application & API routes
├── web.py              # Auth API endpoints (/api/auth/*)
├── federation.py       # Federation protocol
├── json_discovery.py   # Server discovery
├── frontend/           # React SPA (Vite + TypeScript)
│   ├── src/
│   │   ├── App.tsx             # Router & route definitions
│   │   ├── hooks/useAuth.ts    # Auth hook (login, register, 2FA)
│   │   └── pages/
│   │       ├── ChatPage.tsx      # Main chat UI
│   │       ├── LoginPage.tsx     # Login form
│   │       ├── RegisterPage.tsx  # Registration with invite codes
│   │       ├── SetupPage.tsx     # First-time admin setup
│   │       ├── TwoFactorPage.tsx # 2FA verification
│   │       └── AdminPage.tsx     # User & invite management
│   └── vite.config.ts  # Vite config with API proxy
├── Testing/            # Test environment configs
│   ├── NodeA.env
│   ├── NodeB.env
│   └── Channel.env
└── .env                # Local environment config
```

## Multi-Node Testing

To run multiple federated nodes locally:

```bash
# Terminal 1
python app.py Testing/NodeA.env

# Terminal 2
python app.py Testing/NodeB.env
```

Each node gets its own database and port as defined in its env file.
