# Webhooks

Webhooks allow external services to send messages to BSCP as incoming DMs or channel messages. Each webhook has a unique URL that accepts incoming POST requests.

## Creating a Webhook

1. Open **Settings** in the app
2. Scroll to **Webhooks** section
3. Click **Create**
4. Enter webhook name (e.g., "GitHub", "Alerts")
5. Optionally add an avatar URL
6. Click **Create**

Your webhook is created with a unique URL displayed in the list.

## Webhook URL Format

```
http://[domain]/webhooks/[webhook_id]/[webhook_token]
```

Example:
```
http://localhost:5000/webhooks/a1b2c3d4-e5f6-7890-abcd-ef1234567890/ghi3jklm_nopqrst_uvwxyz
```

## Sending Messages

Send a POST request to your webhook URL with JSON payload:

```bash
curl -X POST http://localhost:5000/webhooks/[id]/[token] \
  -H "Content-Type: application/json" \
  -d '{
    "content": "Your message here",
    "username": "Custom Name (optional)",
    "avatar_url": "https://example.com/avatar.png (optional)"
  }'
```

## Payload Format

**Required:**
- `content` (string) - Message text, supports markdown

**Optional:**
- `username` (string) - Override webhook name for this message display
- `avatar_url` (string) - Override webhook avatar for this message (currently not used)

## Examples

**Simple message:**
```json
{
  "content": "Build completed successfully!"
}
```

**With GitHub style:**
```json
{
  "content": "**User123** pushed 3 commits to main\n- Fix bug in auth\n- Add tests\n- Update docs"
}
```

**Markdown support:**
```json
{
  "content": "**Bold text** *italic* `code` [link](https://example.com)"
}
```

## Webhook Management

- **Copy URL** - Copy webhook URL to clipboard
- **Regenerate** - Create new token (old token stops working)
- **Delete** - Permanently remove webhook

Webhook names are immutable. Create a new webhook if you need a different name.

## Channel Webhooks

Channel servers also support webhooks with the same API:

```bash
curl -X POST http://[channel-domain]/webhooks/[id]/[token] \
  -H "Content-Type: application/json" \
  -d '{"content": "Message to channel"}'
```

Create channel webhooks via the channel server API:
```bash
POST /api/channel/webhooks
{
  "path": "domain#channel#subchannel",
  "name": "Webhook Name",
  "avatar_url": "https://example.com/avatar.png"
}
```
