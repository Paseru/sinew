# Sinew Remote relay

This folder is the small relay reachable at `remote.sinew-ide.com`. It is the
**only piece you host**: the desktop app runs on your PC, and the mobile PWA is
served by this relay.

Responsibilities:

- keep an outbound WebSocket from the desktop app;
- route opaque encrypted frames between a paired phone and that PC;
- serve the mobile PWA (`public/`);
- send generic Web Push notifications when the PC reports a completed turn.

It never decrypts chat content. Pairing responses and all runtime
commands/events are AEAD envelopes between the phone and the PC.

## Run locally

```bash
cd remote
npm install
npm start
# → http://localhost:8787
```

## Deploy on Railway

The relay is self-contained (`remote/package.json` only pulls `ws` +
`web-push`), so deploy just this folder:

1. **New Project → Deploy from GitHub repo** and pick this repository.
2. **Settings → Root Directory → `remote`.** Railway (Nixpacks) detects Node,
   runs `npm install`, then `npm start`.
3. **Variables** (Settings → Variables):
   - `VAPID_PUBLIC_KEY`
   - `VAPID_PRIVATE_KEY`
   - `VAPID_SUBJECT` (optional, defaults to `mailto:security@sinew-ide.com`)
   - `PORT` is injected by Railway — do **not** set it.
4. **Networking → Custom Domain → `remote.sinew-ide.com`**, then add the
   `CNAME` Railway gives you at your DNS provider. Railway terminates TLS, so
   the desktop reaches it over `wss://remote.sinew-ide.com/ws`.
5. **Keep a single replica.** Routing state (connected PCs, pairing codes) is
   in-memory, so horizontal scaling would split a phone from its PC. One
   instance is plenty for a PC ↔ a few devices.

Health check: `GET /healthz` → `{ "ok": true }`.

### VAPID keys (push notifications)

```bash
npx web-push generate-vapid-keys
```

Put the public/private keys in the Railway variables above. Without them the
relay still works — only push notifications are disabled (chat, streaming and
pairing are unaffected).

## No database / no volume

The relay is stateless: nothing is persisted, no DB, no disk. On restart both
the desktop and the phone reconnect automatically and resume streaming.
