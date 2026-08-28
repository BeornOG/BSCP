# BSCP
*Beorn's Stupid Chat Protocol* is designed as a federated discord/group chat alternative. It has been designed so that the original sender is responsable for making media available via embedding urls. it also is configured that any recieved media is proxied, and cached by the recuevers user server to prevent media being used to get other uses ip's.

the goal it to also have channel servers. these are intended as replacements for discord guilds. these are intended to store messages in channels and providing the ability for only some people to get acces to certain channels.

a username is defined as ``[user]@[domain]``
a channel is defined as ``[domain]#[channel]#[subchannel]#[subchannel]``

the protocol also supports /.well-known/BSCP/ for usecases where you want to use a domain but don't want the federation api there.

## Prerequisites

- **Rust 1.97+** (`rustup`), for building the servers
- **Node.js 18+**, only needed to compile the front-end (not needed if you pre-compile it on another device)

# userserver

## Frontend Setup

1. Install dependencies:

   ```bash
   cd frontend
   npm install
   ```

2. Build the SPA (output goes to `../static/`, which the user server serves):

   ```bash
   npm exec vite build
   ```

## Backend Setup

The backend is a Cargo workspace with three crates:

| Crate | Binary | Purpose |
|---|---|---|
| `crates/common` | — | shared config, DB, models, auth, federation, push |
| `crates/userserver` | `bscp-userserver` | the user server (`/api/*`, `/federation/*`, media proxy, SPA) |
| `crates/channelserver` | `bscp-channelserver` | the channel server (`/api/channel/*`) |

1. Create a `.env` file in the project root (or use one of the test configs in `Testing/`):

   ```env
   PORT=5000
   DOMAIN=localhost:5000
   DB_NAME=data/userserver.db
   CACHE_DIR=media_cache
   CACHE_TIME=86400
   UPLOAD_DIR=uploads
   SECRET_KEY=change-me
   ```

2. Run the user server (development build — this build also serves the Swagger UI at
   `/api/docs/`):

   ```bash
   cargo run -p bscp-userserver -- Testing/NodeA.env
   ```

   Positional arg = env file (default `.env`); `--db <path>` overrides `DB_NAME`.

3. For deployment, build the optimised binary (the `/api/docs/` endpoint is compiled out of
   release builds):

   ```bash
   cargo build --release
   ./target/release/bscp-userserver .env
   ```

The database schema is created automatically on first run via embedded migrations
(`crates/userserver/migrations/`). SQLite files are created if missing.

### Channel server

```bash
cargo run -p bscp-channelserver -- Testing/Channel.env
```

Reads `CH_PORT` (default 6000), `CH_DB_NAME` (default `data/channelserver.db`) and `DOMAIN`.

## First-Time Setup

1. Open `http://localhost:5000` in your browser.
2. You'll be redirected to the setup page to create an admin account.
3. After setup, log in with your new credentials.

## Tests

```bash
cargo test --workspace
```

## Project Structure

```
BSCP/
├── Cargo.toml               # workspace
├── rust-toolchain.toml
├── crates/
│   ├── common/              # shared library (bscp-common)
│   ├── userserver/          # bscp-userserver binary + migrations/ + openapi.json
│   │   └── src/routes/      # auth, users, chats, uploads, invites, webhooks, admin,
│   │                        #   federation, misc (media proxy, .well-known, SPA)
│   └── channelserver/       # bscp-channelserver binary + migrations/
├── frontend/                # React SPA (Vite + TypeScript), builds to ../static/
├── Testing/                 # Test environment configs (NodeA/NodeB/Channel .env)
└── static/                  # built frontend (git-ignored)
```

## Multi-Node Testing

To run multiple federated nodes locally:

```bash
# Terminal 1
cargo run -p bscp-userserver -- Testing/NodeA.env

# Terminal 2
cargo run -p bscp-userserver -- Testing/NodeB.env
```

Each node gets its own database and port as defined in its env file.

## Webhooks

See [WEBHOOKS.md](WEBHOOKS.md) for documentation on creating and using webhooks to send messages via external services.
