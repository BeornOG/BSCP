# BSCP
*Beorn's Stupid Chat Protocol* is designed as a federated discord/group chat alternative. It has been designed so that the original sender is responsable for making media available via embedding urls. it also is configured that any recieved media is proxied, and cached by the recuevers user server to prevent media being used to get other uses ip's.

the goal it to also have channel servers. these are intended as replacements for discord guilds. these are intended to store messages in channels and providing the ability for only some people to get acces to certain channels.

a username is defined as ``[user]@[domain]``
a channel is defined as ``[domain]#[channel]#[subchannel]#[subchannel]``

the protocol also supports /.well-known/BSCP/ for usecases where you want to use a domain but don't want the federation api there.

## Voice calls

Audio calls follow the same privacy rule as the media proxy: a browser only ever connects to
**its own user server**. Every call has one *manager* that relays signaling (SDP/ICE) but never
media — for a DM call the manager runs on the caller's user server; for a channel voice room it
runs on the channel server. Audio flows browser ⇄ own user server ⇄ (mesh of) the other
participants' user servers ⇄ their browsers; each user server is a small Opus SFU. A DM call is
started by sending a `call_invite` chat message whose metadata carries the manager address;
the callee's client shows a ring and connects on accept. Channel rooms are join-by-choice.

Config: `ICE_PUBLIC_IP` (this server's public IP for 1:1 NAT) and `RTC_PORT_MIN`/`RTC_PORT_MAX`
(the UDP range to open). No external STUN/TURN is needed as long as the user servers are
mutually reachable (which federation already requires).

## Channel servers — guilds

A channel server hosts many **guilds** (Discord-style servers), each a tree of **channels**
(`text` / `voice` / `category`, nestable). Everything is UUID-keyed — the path is
`domain#<guild-uuid>#<channel-uuid>#…` and names are mutable metadata.

**The browser only ever talks to its own user server**, which proxies guild/channel/message
traffic to the channel server (`/api/gw/<channel-server>/…`) and polls it for updates. The
known trade-off: a malicious user-server operator could snoop a user's guilds.

**Auth is a federation assertion — automatic and mutual.** When a member's user server needs
to call a channel server, it silently mints a short-lived RS256 JWT
(`iss`=userserver, `sub`=`user@domain`, `aud`=channel-server) with its OIDC signing key. The
channel server verifies the signature against the issuer's JWKS **and** calls back
(`POST {iss}/federation/assert/verify`) to confirm the issuer really minted it — replay /
key-substitution both fail unless an attacker controls the issuer domain's TLS. There is no
OIDC consent prompt: **joining the guild is the authorization.** The only production
requirement is valid TLS certs on both servers.

**Permissions** are Discord-like: server roles carry a permission bitmask (`VIEW_CHANNEL`,
`SEND_MESSAGES`, `CONNECT`, `MANAGE_CHANNELS`, `MANAGE_ROLES`, `ADMINISTRATOR`, …); each
channel can `allow`/`deny` per role or per member. Effective = owner→all, else
(`@everyone` ∪ roles), `ADMINISTRATOR` short-circuits, then channel overrides
(`@everyone` → roles → member), then a `VIEW_CHANNEL` gate.

**Voice channels** reuse the call mesh: the channel server is the room's *manager* (signaling
only, persistent, join-by-choice), member user servers relay audio directly — `CONNECT`-gated.

**Invites** are shareable links: `https://<channel-server>/invite/<code>`. Opening one shows a
landing page that bounces the visitor to `https://<their-server>/join?invite=…`; one click and
their user server does the assertion + accept.

### Operator console

The channel server serves its own minimal console at `/`. On first run the operator signs in
with **"Sign in with BSCP"** against their home user server (the channel server is an OIDC
client, dynamically registered) — the first person to complete it claims the operator role.
The console manages the **guild-creator allowlist** (`user@domain` identities permitted to
create guilds) and shows a table of guilds ↔ owners. The allowlist can also be seeded from
config: `CH_ALLOW_GUILD_CREATORS=alice@a.example,bob@b.example`.

Channel-server config: `CH_PUBLIC_URL` (externally reachable base URL, for the OIDC
`redirect_uri` and invite links; default `http://{DOMAIN}`), `CH_SECRET_KEY` (operator cookie),
plus the existing `CH_PORT` / `CH_DB_NAME` / `DOMAIN`.

## Sign in with BSCP (OIDC)

Every user server is its own **OpenID Connect provider** — there is no central issuer. An app
lets the user type `alice@alice.example`, discovers that server through
`/.well-known/BSCP/userserver` (the new `oidc` block → `/.well-known/openid-configuration`),
and runs a standard auth-code + PKCE flow against it. The server only ever authenticates its
**own local users** (via the session cookie), so `sub` is always `localuser@thisdomain`;
cross-domain identity is the relying party's job.

Relying parties become trusted in one of two ways:

- **Dynamic registration** — `POST /oauth/register` (RFC 7591), once per home-server domain.
- **Federation trust** — an unregistered `client_id` that is an `https://` origin is accepted
  if `<client_id>/.well-known/BSCP/relying-party` lists the exact `redirect_uri`
  (`{ "client_name", "redirect_uris": [...] }`). PKCE required, no secret, consent always shown.

Endpoints: `/.well-known/openid-configuration`, `/oauth/jwks` (RS256), `/oauth/register`,
`/oauth/authorize` (+ a server-rendered consent page), `/oauth/token`, `/oauth/userinfo`,
`/oauth/revoke`. Config: `PUBLIC_URL` (the externally reachable base URL — the issuer;
defaults from `DOMAIN`), `OIDC_ACCESS_TTL`, `OIDC_REFRESH_TTL`. The signing key is generated
once into `oidc_keys.json`. Admins can disable the provider or revoke clients in the admin
panel.

## Modules

Server owners can install **out-of-process modules** — separate HTTP services registered in
the admin panel by base URL (the server generates a shared secret and fetches the module's
`/.well-known/bscp-module` manifest). A module:

- receives **signed event webhooks** (`X-BSCP-Signature: sha256=<hmac>`) for the events it
  subscribed to: `user.registered`, `user.deleted`, `session.created`, `message.sent`,
  `message.received`, `webhook.received` (message events include content — shown at install);
- can offer **external account linking**: it declares `link_providers` (e.g. GitHub); the
  user clicks *Connect* under Settings → Connections, the server hands the module a signed
  ticket, the module runs its own OAuth and calls back `POST /api/modules/<name>/links`
  (module-signed) to record the link. Linked accounts appear on the profile and, with the
  `bscp:links` scope, in OIDC `userinfo`.

Modules get no routes into the user server and no injected SPA UI.

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

   # OIDC issuer / absolute redirects. Defaults to https://DOMAIN (http:// for localhost).
   PUBLIC_URL=https://alice.example

   # Voice calls (WebRTC). Optional in dev; required for calls across NAT.
   ICE_PUBLIC_IP=203.0.113.10   # this server's public IP (1:1 NAT)
   RTC_PORT_MIN=50000           # open this UDP range on the firewall
   RTC_PORT_MAX=50100
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
