//! Environment / CLI configuration, mirroring the behaviour of the old `app.py`.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct UserServerConfig {
    pub port: u16,
    pub domain: String,
    pub db_path: PathBuf,
    pub secret_key: String,
    pub cache_dir: PathBuf,
    pub cache_time: u64,
    pub upload_dir: PathBuf,
    pub static_dir: PathBuf,
    pub vapid_public_key: String,
    pub vapid_private_key: String,
    pub vapid_contact: String,
    /// Path where an auto-generated VAPID keypair is persisted.
    pub vapid_keys_file: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ChannelServerConfig {
    pub port: u16,
    pub domain: String,
    pub db_path: PathBuf,
}

/// Parsed command line: `<binary> [env_file] [--db=PATH | --db PATH]`.
pub struct Args {
    pub env_file: Option<String>,
    pub db_override: Option<String>,
}

impl Args {
    pub fn parse() -> Self {
        let raw: Vec<String> = std::env::args().skip(1).collect();
        let mut env_file = None;
        let mut db_override = None;
        let mut i = 0;
        while i < raw.len() {
            let arg = &raw[i];
            if let Some(v) = arg.strip_prefix("--db=") {
                db_override = Some(v.to_string());
            } else if arg == "--db" {
                if i + 1 < raw.len() {
                    db_override = Some(raw[i + 1].clone());
                    i += 1;
                }
            } else if !arg.starts_with("--") && env_file.is_none() {
                env_file = Some(arg.clone());
            }
            i += 1;
        }
        Args { env_file, db_override }
    }
}

fn basedir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Load the given dotenv file into the process environment. A path explicitly
/// provided on the command line must exist; the default `.env` is optional.
fn load_env(env_file: &Option<String>) -> anyhow::Result<String> {
    let name = env_file.clone().unwrap_or_else(|| ".env".to_string());
    let path = basedir().join(&name);
    if env_file.is_some() && !path.exists() {
        anyhow::bail!("Specified env file not found: {}", path.display());
    }
    // Does not override variables already present in the real environment
    // (matches python-dotenv's default behaviour).
    let _ = dotenvy::from_path(&path);
    Ok(name)
}

fn env_str(key: &str) -> Option<String> {
    std::env::var(key).ok().map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Read an integer env var, tolerating trailing inline `#` comments / whitespace.
fn env_int<T: std::str::FromStr>(key: &str, default: T) -> T {
    env_str(key)
        .and_then(|v| {
            let head: String = v.chars().take_while(|c| !c.is_whitespace() && *c != '#').collect();
            head.parse().ok()
        })
        .unwrap_or(default)
}

fn resolve_path(base: &Path, value: &str) -> PathBuf {
    let p = PathBuf::from(value);
    if p.is_absolute() { p } else { base.join(p) }
}

fn ensure_parent_dir(path: &Path) {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(parent);
        }
    }
}

impl UserServerConfig {
    pub fn load(args: &Args) -> anyhow::Result<Self> {
        let env_name = load_env(&args.env_file)?;
        let base = basedir();

        let port: u16 = env_int("PORT", 5000);
        let domain = env_str("DOMAIN").unwrap_or_else(|| format!("localhost:{port}"));

        let db_path = match &args.db_override {
            Some(v) => resolve_path(&base, v),
            None => resolve_path(&base, &env_str("DB_NAME").unwrap_or_else(|| "data/userserver.db".into())),
        };
        let cache_dir = resolve_path(&base, &env_str("CACHE_DIR").unwrap_or_else(|| "media_cache".into()));
        let upload_dir = resolve_path(&base, &env_str("UPLOAD_DIR").unwrap_or_else(|| "uploads".into()));
        let cache_time: u64 = env_int("CACHE_TIME", 3600);

        ensure_parent_dir(&db_path);
        let _ = std::fs::create_dir_all(&cache_dir);
        let _ = std::fs::create_dir_all(&upload_dir);

        let cfg = UserServerConfig {
            port,
            domain,
            db_path,
            secret_key: env_str("SECRET_KEY").unwrap_or_else(|| "default_secret_key".into()),
            cache_dir,
            cache_time,
            upload_dir,
            static_dir: base.join("static"),
            vapid_public_key: env_str("VAPID_PUBLIC_KEY").unwrap_or_default(),
            vapid_private_key: env_str("VAPID_PRIVATE_KEY").unwrap_or_default(),
            vapid_contact: env_str("VAPID_CONTACT").unwrap_or_else(|| "mailto:admin@localhost".into()),
            vapid_keys_file: base.join("vapid_keys.json"),
        };

        tracing::info!(config = %env_name, domain = %cfg.domain, port = cfg.port,
            db = %cfg.db_path.display(), cache_dir = %cfg.cache_dir.display(),
            cache_time = cfg.cache_time, upload_dir = %cfg.upload_dir.display(),
            "UserNode configuration");
        Ok(cfg)
    }

    pub fn database_url(&self) -> String {
        format!("sqlite://{}?mode=rwc", self.db_path.to_string_lossy().replace('\\', "/"))
    }
}

impl ChannelServerConfig {
    pub fn load(args: &Args) -> anyhow::Result<Self> {
        let env_name = load_env(&args.env_file)?;
        let base = basedir();

        let port: u16 = env_int("CH_PORT", 6000);
        let domain = env_str("DOMAIN").unwrap_or_else(|| format!("localhost:{port}"));
        let db_path = match &args.db_override {
            Some(v) => resolve_path(&base, v),
            None => resolve_path(&base, &env_str("CH_DB_NAME").unwrap_or_else(|| "data/channelserver.db".into())),
        };
        ensure_parent_dir(&db_path);

        tracing::info!(config = %env_name, domain = %domain, port = port,
            db = %db_path.display(), "ChannelNode configuration");
        Ok(ChannelServerConfig { port, domain, db_path })
    }

    pub fn database_url(&self) -> String {
        format!("sqlite://{}?mode=rwc", self.db_path.to_string_lossy().replace('\\', "/"))
    }
}
