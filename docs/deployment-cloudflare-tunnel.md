# Cloudflare Tunnel — robo-fleet WebSocket Deployment

Exposes orchestra's `web_bridge` (Socket.IO, port 3030) via a Cloudflare Tunnel subdomain so browsers on HTTPS pages can connect with `wss://` instead of `ws://` (which browsers block as mixed content).

```
Browser (HTTPS)
    ↓ wss://robo-fleet.{domain}/socket.io/?...
Cloudflare Edge (TLS termination)
    ↓ QUIC tunnel
cloudflared (same host as orchestra)
    ↓ http://localhost:3030/socket.io/?...
Orchestra web_bridge (Rust Socket.IO)
```

## Prerequisites

- Cloudflare Tunnel already running for the main domain (e.g. `dms-study.cloud`)
- `cloudflared` installed and authenticated on the server
- `cloudflared` service managed by `systemd`

## Setup: Add robo-fleet Subdomain to Existing Tunnel

Run from the **server** hosting `cloudflared` (not local dev machine):

```bash
cd /path/to/qm-sync/qm-hub-server
./scripts/setup-cloudflare-tunnel.sh qm-hub "" "" robo-fleet.dms-study.cloud --patch
```

The script:
1. Routes DNS `robo-fleet.dms-study.cloud` → `<tunnel-id>.cfargotunnel.com` via `cloudflared tunnel route dns`
2. Injects ingress rule into `/etc/cloudflared/config.yml` before the catch-all (idempotent — skips if hostname already present)
3. Validates the patched config
4. Restarts `cloudflared` via `systemctl`

## Manual Steps (if script unavailable)

### 1. Add DNS route

```bash
# Find tunnel name
cloudflared tunnel list

# Route DNS
cloudflared tunnel route dns <TUNNEL_NAME> robo-fleet.dms-study.cloud
```

Alternatively: Cloudflare Dashboard → DNS → Add CNAME `robo-fleet` → `<tunnel-id>.cfargotunnel.com` (Proxied).

### 2. Patch `/etc/cloudflared/config.yml`

Add before the catch-all line (`- service: http_status:404`):

```yaml
  # Orchestra web_bridge (WebSocket + HTTP)
  - hostname: robo-fleet.dms-study.cloud
    service: http://localhost:3030
    originRequest:
      connectTimeout: 30s
      noHappyEyeballs: true

  # Catch-all (must remain last)
  - service: http_status:404
```

### 3. Validate and restart

```bash
sudo cloudflared tunnel ingress validate --config /etc/cloudflared/config.yml
sudo systemctl restart cloudflared
sudo journalctl -u cloudflared -f --since "1 min ago"
```

## Verify WebSocket Connectivity

```bash
# HTTP polling transport
curl -v "https://robo-fleet.dms-study.cloud/socket.io/?EIO=4&transport=polling"

# WebSocket transport (requires wscat: npm i -g wscat)
wscat -c "wss://robo-fleet.dms-study.cloud/socket.io/?EIO=4&transport=websocket"
```

Both should connect without TLS errors.

## Update App Environment

After the tunnel is confirmed live, update `apps/web/.env` (and `apps/native/.env` if deploying Tauri):

```env
VITE_SOCKET_IO_URL=https://robo-fleet.dms-study.cloud
```

> Use `https://` (not `wss://`) — Socket.IO auto-upgrades the transport to WebSocket. Passing `https://` also avoids mixed content issues if Socket.IO falls back to polling.

Then rebuild and redeploy:

```bash
pnpm build
```

## Update CORS on Orchestra Side

The browser's `Origin` header is set to the **page** origin (e.g. `https://dms-study.cloud`), not the WebSocket target. Ensure orchestra's `ALLOWED_ORIGINS` includes the page origin:

```yaml
# robo-fleet-dora-rs/docker/docker-compose.yml
ALLOWED_ORIGINS: "${ALLOWED_ORIGINS:-https://dms-study.cloud,http://localhost:1420,http://localhost:3000,http://localhost:5173}"
```

Rebuild orchestra after changing:

```bash
docker compose --profile orchestra up -d --build
```

## Domain Portability

If the domain changes from `dms-study.cloud`:

| Location | What to change |
|----------|---------------|
| `/etc/cloudflared/config.yml` | `hostname:` field for the robo-fleet ingress rule |
| Cloudflare DNS | CNAME record (or re-run `cloudflared tunnel route dns`) |
| `apps/web/.env` | `VITE_SOCKET_IO_URL` |
| `docker-compose.yml` | `ALLOWED_ORIGINS` (or set the env var) |

## Operational Notes

| Concern | Detail |
|---------|--------|
| WebSocket timeout | Cloudflare hard-limits idle connections at ~100s. Socket.IO's default heartbeat (25s) keeps connections alive. |
| Message size limit | Cloudflare caps WebSocket messages at 100MB. JPEG frames (~50–100KB) are well under. |
| Latency | Cloudflare edge adds <20ms typically. Acceptable for real-time rover control. |
| Auth bypass | Cloudflare Tunnel only handles TLS+routing — orchestra's username/password auth is unaffected. |
| Additional access control | Add Cloudflare Access (Zero Trust) on the subdomain if public exposure is a concern. |
