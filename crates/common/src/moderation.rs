//! Federated-domain blocklist shared by the user server and channel server
//! (issue #8). Both keep a `blocked_domains(domain TEXT PRIMARY KEY, …)` table;
//! this module normalises input and answers "is this domain blocked?".

use sqlx::SqlitePool;

/// Normalise a domain for storage and comparison: lower-case, strip any scheme,
/// `user@` prefix, port and path, and a trailing dot.
pub fn normalize_domain(input: &str) -> String {
    let mut s = input.trim().to_ascii_lowercase();
    for scheme in ["https://", "http://"] {
        if let Some(rest) = s.strip_prefix(scheme) {
            s = rest.to_string();
        }
    }
    if let Some((_, host)) = s.rsplit_once('@') {
        s = host.to_string();
    }
    s.split(['/', ':'])
        .next()
        .unwrap_or(&s)
        .trim_end_matches('.')
        .to_string()
}

/// Is `domain` (a bare domain, `user@domain`, or a URL) on the blocklist?
pub async fn is_domain_blocked(pool: &SqlitePool, domain: &str) -> bool {
    let d = normalize_domain(domain);
    if d.is_empty() {
        return false;
    }
    sqlx::query_scalar::<_, i64>("SELECT 1 FROM blocked_domains WHERE domain = ? LIMIT 1")
        .bind(&d)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .is_some()
}

#[cfg(test)]
mod tests {
    use super::normalize_domain;

    #[test]
    fn strips_scheme_port_path_and_user() {
        assert_eq!(normalize_domain("https://Evil.Example.com:8080/foo"), "evil.example.com");
        assert_eq!(normalize_domain("alice@spam.test"), "spam.test");
        assert_eq!(normalize_domain("  Spam.Test.  "), "spam.test");
    }
}
