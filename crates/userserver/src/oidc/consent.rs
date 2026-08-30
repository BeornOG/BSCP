//! Server-rendered consent page for `/oauth/authorize`.

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn scope_label(s: &str) -> &'static str {
    match s {
        "openid" => "Confirm your identity",
        "profile" => "Your display name and avatar",
        "email" => "Your email address",
        "bscp:links" => "Your linked external accounts",
        _ => "Additional access",
    }
}

/// `hidden_fields` is pre-rendered `<input type=hidden …>` markup for every
/// authorize parameter so the POST carries them back.
pub fn page(
    client_name: &str,
    logo_url: Option<&str>,
    subject: &str,
    scopes: &[&str],
    hidden_fields: &str,
    csrf: &str,
    action: &str,
) -> String {
    let logo = logo_url
        .map(|u| format!("<img src=\"{}\" alt=\"\" class=\"logo\">", esc(u)))
        .unwrap_or_default();
    let scope_items: String = scopes
        .iter()
        .filter(|s| **s != "openid")
        .map(|s| format!("<li>{}</li>", esc(scope_label(s))))
        .collect();
    let scope_block = if scope_items.is_empty() {
        String::new()
    } else {
        format!("<p class=\"muted\">This will share:</p><ul>{scope_items}</ul>")
    };

    format!(
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Authorize {name}</title>
<style>
:root{{color-scheme:dark}}
body{{margin:0;min-height:100vh;display:flex;align-items:center;justify-content:center;
background:#0a0a0b;color:#e8eaed;font:15px/1.5 system-ui,sans-serif}}
.card{{width:340px;background:#151517;border:1px solid #232529;border-radius:16px;padding:28px;text-align:center}}
.logo{{width:48px;height:48px;border-radius:12px;object-fit:cover;margin-bottom:8px}}
h1{{font-size:18px;margin:.2rem 0}}
.muted{{color:#8a8d93;font-size:13px;margin:.6rem 0 .3rem}}
.sub{{font-family:ui-monospace,monospace;color:#7eafff;font-size:13px;word-break:break-all}}
ul{{text-align:left;margin:.3rem 0 1rem;padding-left:1.1rem;color:#c9ccd1;font-size:13px}}
.row{{display:flex;gap:10px;margin-top:16px}}
button{{flex:1;padding:10px;border:0;border-radius:9px;font:inherit;font-weight:600;cursor:pointer}}
.deny{{background:#232529;color:#e8eaed}}
.allow{{background:#6e8efb;color:#fff}}
</style></head><body>
<form class="card" method="post" action="{action}">
{logo}
<h1>{name}</h1>
<p class="muted">wants to sign you in as</p>
<p class="sub">{subject}</p>
{scope_block}
{hidden}
<input type="hidden" name="csrf" value="{csrf}">
<div class="row">
<button class="deny" name="decision" value="deny" type="submit">Deny</button>
<button class="allow" name="decision" value="approve" type="submit">Allow</button>
</div>
</form></body></html>"#,
        name = esc(client_name),
        subject = esc(subject),
        action = esc(action),
        csrf = esc(csrf),
        hidden = hidden_fields,
        logo = logo,
        scope_block = scope_block,
    )
}
