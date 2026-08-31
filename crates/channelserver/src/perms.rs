//! Guild permission bitmask + effective-permission resolution.

use crate::state::AppState;

pub const VIEW_CHANNEL: u64 = 1 << 0;
pub const SEND_MESSAGES: u64 = 1 << 1;
pub const MANAGE_MESSAGES: u64 = 1 << 2;
pub const CONNECT: u64 = 1 << 3;
pub const SPEAK: u64 = 1 << 4;
pub const MANAGE_CHANNELS: u64 = 1 << 5;
pub const MANAGE_ROLES: u64 = 1 << 6;
pub const MANAGE_GUILD: u64 = 1 << 7;
pub const KICK_MEMBERS: u64 = 1 << 8;
pub const CREATE_INVITE: u64 = 1 << 9;
pub const ADMINISTRATOR: u64 = 1 << 10;

pub const ALL: u64 = (1 << 11) - 1;

/// Sensible starting perms for the auto-created `@everyone` role.
pub const EVERYONE_DEFAULT: u64 =
    VIEW_CHANNEL | SEND_MESSAGES | CONNECT | SPEAK | CREATE_INVITE;

pub fn has(mask: u64, perm: u64) -> bool {
    mask & ADMINISTRATOR != 0 || mask & perm == perm
}

/// Effective permissions for `user` in `guild`, optionally narrowed to `channel`.
pub async fn effective(state: &AppState, guild_id: &str, user: &str, channel_id: Option<&str>) -> u64 {
    // owner → everything
    let owner: Option<String> = sqlx::query_scalar("SELECT owner FROM guilds WHERE id = ?")
        .bind(guild_id)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten();
    if owner.as_deref() == Some(user) {
        return ALL;
    }
    // must be a member
    let is_member: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM guild_members WHERE guild_id = ? AND user_id = ?")
            .bind(guild_id)
            .bind(user)
            .fetch_optional(&state.pool)
            .await
            .ok()
            .flatten();
    if is_member.is_none() {
        return 0;
    }

    // base = @everyone ∪ assigned roles
    let role_perms: Vec<(String, i64, i64)> = sqlx::query_as(
        "SELECT r.id, r.permissions, r.is_everyone FROM roles r \
         WHERE r.guild_id = ? AND (r.is_everyone = 1 OR r.id IN \
           (SELECT role_id FROM member_roles WHERE guild_id = ? AND user_id = ?))",
    )
    .bind(guild_id)
    .bind(guild_id)
    .bind(user)
    .fetch_all(&state.pool)
    .await
    .unwrap_or_default();

    let mut base: u64 = 0;
    let mut my_role_ids: Vec<String> = Vec::new();
    for (rid, perms, _is_everyone) in &role_perms {
        base |= *perms as u64;
        my_role_ids.push(rid.clone());
    }
    if base & ADMINISTRATOR != 0 {
        return ALL;
    }

    let Some(channel_id) = channel_id else { return base };

    let everyone_id: Option<String> =
        sqlx::query_scalar("SELECT id FROM roles WHERE guild_id = ? AND is_everyone = 1")
            .bind(guild_id)
            .fetch_optional(&state.pool)
            .await
            .ok()
            .flatten();

    // Resolve overrides in Discord order: the parent category first (channels
    // inherit their category's overrides — a hidden "Staff" category hides every
    // channel inside it), then the channel's own overrides on top.
    let parent_id: Option<String> = sqlx::query_scalar("SELECT parent_id FROM channels WHERE id = ?")
        .bind(channel_id)
        .fetch_optional(&state.pool)
        .await
        .ok()
        .flatten()
        .flatten();

    let mut p = base;
    for scope in parent_id.as_deref().into_iter().chain(std::iter::once(channel_id)) {
        let overrides: Vec<(String, String, i64, i64)> = sqlx::query_as(
            "SELECT target_type, target_id, allow, deny FROM channel_overrides WHERE channel_id = ?",
        )
        .bind(scope)
        .fetch_all(&state.pool)
        .await
        .unwrap_or_default();
        p = apply_scope(p, &overrides, everyone_id.as_deref(), &my_role_ids, user);
    }

    if p & VIEW_CHANNEL == 0 {
        return 0;
    }
    p
}

/// Apply one channel (or category) override layer: `@everyone` → union of the
/// member's roles → member-specific, each as `p = (p & !deny) | allow`.
fn apply_scope(
    mut p: u64,
    overrides: &[(String, String, i64, i64)],
    everyone_id: Option<&str>,
    my_role_ids: &[String],
    user: &str,
) -> u64 {
    let apply = |p: u64, allow: u64, deny: u64| (p & !deny) | allow;

    if let Some(eid) = everyone_id {
        for (tt, tid, a, d) in overrides {
            if tt == "role" && tid == eid {
                p = apply(p, *a as u64, *d as u64);
            }
        }
    }
    // accumulate role allow/deny then apply once (Discord semantics)
    let (mut role_allow, mut role_deny) = (0u64, 0u64);
    for (tt, tid, a, d) in overrides {
        if tt == "role" && my_role_ids.iter().any(|r| r == tid) {
            role_allow |= *a as u64;
            role_deny |= *d as u64;
        }
    }
    p = apply(p, role_allow, role_deny);
    for (tt, tid, a, d) in overrides {
        if tt == "member" && tid == user {
            p = apply(p, *a as u64, *d as u64);
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    #[test]
    fn has_respects_administrator() {
        assert!(has(ADMINISTRATOR, MANAGE_GUILD));
        assert!(has(SEND_MESSAGES | VIEW_CHANNEL, SEND_MESSAGES));
        assert!(!has(VIEW_CHANNEL, SEND_MESSAGES));
    }

    async fn test_state() -> AppState {
        let pool = SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        crate::MIGRATOR.run(&pool).await.unwrap();
        let cfg = bscp_common::config::ChannelServerConfig {
            port: 0,
            domain: "chan.test".into(),
            db_path: std::env::temp_dir().join("x"),
            public_url: "http://chan.test".into(),
            secret_key: "k".into(),
            keys_file: std::env::temp_dir().join("k.json"),
        };
        AppState::new(pool, &cfg)
    }

    #[tokio::test]
    async fn effective_permissions_matrix() {
        let st = test_state().await;
        let p = &st.pool;
        let now = bscp_common::now_ts();
        sqlx::query("INSERT INTO guilds (id,name,owner,created_at) VALUES ('g','G','owner@a',?)").bind(now).execute(p).await.unwrap();
        sqlx::query("INSERT INTO roles (id,guild_id,name,permissions,is_everyone) VALUES ('everyone','g','@everyone',?,1)")
            .bind((VIEW_CHANNEL | SEND_MESSAGES) as i64).execute(p).await.unwrap();
        sqlx::query("INSERT INTO roles (id,guild_id,name,permissions,is_everyone) VALUES ('mod','g','mod',?,0)")
            .bind(MANAGE_MESSAGES as i64).execute(p).await.unwrap();
        sqlx::query("INSERT INTO channels (id,guild_id,name,kind,path) VALUES ('c','g','gen','text','chan.test#g#c')").execute(p).await.unwrap();
        for u in ["owner@a", "alice@a", "bob@b", "carol@c"] {
            sqlx::query("INSERT INTO guild_members (guild_id,user_id,joined_at) VALUES ('g',?,?)").bind(u).bind(now).execute(p).await.unwrap();
        }
        sqlx::query("INSERT INTO member_roles (guild_id,user_id,role_id) VALUES ('g','alice@a','mod')").execute(p).await.unwrap();

        // owner → everything
        assert_eq!(effective(&st, "g", "owner@a", Some("c")).await, ALL);
        // @everyone only
        let bob = effective(&st, "g", "bob@b", Some("c")).await;
        assert!(has(bob, SEND_MESSAGES) && !has(bob, MANAGE_MESSAGES));
        // alice has @everyone ∪ mod
        assert!(has(effective(&st, "g", "alice@a", Some("c")).await, MANAGE_MESSAGES));
        // non-member → nothing
        assert_eq!(effective(&st, "g", "stranger@x", Some("c")).await, 0);

        // channel override: deny SEND to @everyone
        sqlx::query("INSERT INTO channel_overrides (channel_id,target_type,target_id,allow,deny) VALUES ('c','role','everyone',0,?)")
            .bind(SEND_MESSAGES as i64).execute(p).await.unwrap();
        assert!(!has(effective(&st, "g", "bob@b", Some("c")).await, SEND_MESSAGES));
        // ...but allow it back for the mod role
        sqlx::query("INSERT INTO channel_overrides (channel_id,target_type,target_id,allow,deny) VALUES ('c','role','mod',?,0)")
            .bind(SEND_MESSAGES as i64).execute(p).await.unwrap();
        assert!(has(effective(&st, "g", "alice@a", Some("c")).await, SEND_MESSAGES));

        // deny VIEW to @everyone at channel → no access at all
        sqlx::query("UPDATE channel_overrides SET deny = ? WHERE channel_id='c' AND target_id='everyone'")
            .bind((SEND_MESSAGES | VIEW_CHANNEL) as i64).execute(p).await.unwrap();
        assert_eq!(effective(&st, "g", "bob@b", Some("c")).await, 0);
    }

    #[tokio::test]
    async fn staff_category_hides_child_channels() {
        let st = test_state().await;
        let p = &st.pool;
        let now = bscp_common::now_ts();
        sqlx::query("INSERT INTO guilds (id,name,owner,created_at) VALUES ('g','G','owner@a',?)").bind(now).execute(p).await.unwrap();
        sqlx::query("INSERT INTO roles (id,guild_id,name,permissions,is_everyone) VALUES ('everyone','g','@everyone',?,1)")
            .bind((VIEW_CHANNEL | SEND_MESSAGES) as i64).execute(p).await.unwrap();
        sqlx::query("INSERT INTO roles (id,guild_id,name,permissions,is_everyone) VALUES ('staff','g','staff',0,0)")
            .execute(p).await.unwrap();
        // a "Staff" category with a text channel nested under it
        sqlx::query("INSERT INTO channels (id,guild_id,name,kind,path) VALUES ('cat','g','Staff','category','chan.test#g#cat')").execute(p).await.unwrap();
        sqlx::query("INSERT INTO channels (id,guild_id,parent_id,name,kind,path) VALUES ('sc','g','cat','staff-chat','text','chan.test#g#cat#sc')").execute(p).await.unwrap();
        for u in ["alice@a", "bob@b"] {
            sqlx::query("INSERT INTO guild_members (guild_id,user_id,joined_at) VALUES ('g',?,?)").bind(u).bind(now).execute(p).await.unwrap();
        }
        sqlx::query("INSERT INTO member_roles (guild_id,user_id,role_id) VALUES ('g','alice@a','staff')").execute(p).await.unwrap();

        // hide the category from @everyone, grant it back to the staff role — on the category only
        sqlx::query("INSERT INTO channel_overrides (channel_id,target_type,target_id,allow,deny) VALUES ('cat','role','everyone',0,?)")
            .bind(VIEW_CHANNEL as i64).execute(p).await.unwrap();
        sqlx::query("INSERT INTO channel_overrides (channel_id,target_type,target_id,allow,deny) VALUES ('cat','role','staff',?,0)")
            .bind(VIEW_CHANNEL as i64).execute(p).await.unwrap();

        // child channel inherits: staff sees it, everyone else does not
        assert!(has(effective(&st, "g", "alice@a", Some("sc")).await, VIEW_CHANNEL));
        assert_eq!(effective(&st, "g", "bob@b", Some("sc")).await, 0);
        // and the category node itself
        assert!(has(effective(&st, "g", "alice@a", Some("cat")).await, VIEW_CHANNEL));
        assert_eq!(effective(&st, "g", "bob@b", Some("cat")).await, 0);
    }
}
