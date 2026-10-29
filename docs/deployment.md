# Hosting

The repository contains a three-service Compose stack: a static web host with
Caddy HTTPS/WSS termination, ephemeral Axum signaling, and Coturn relay. Live
documents are held by participants. Neither the static host nor signaling is a
cloud document store.

## Prepare a host

Use an amd64 Linux server with Docker Compose, a public IP and DNS records for
ConV and its TURN hostname. Open TCP 80/443, TCP+UDP 3478, and UDP 49160–49200.
The hostname and browser Origin must match. Behind NAT, configure the TURN public
address mapping correctly. Some enterprise networks need a separately configured
TURN-over-TLS endpoint; the supplied relay uses UDP/TCP 3478.

```sh
cp .env.example .env
# Set CONV_HOST, CONV_TURN_HOST, CONV_PUBLIC_IP and a fresh secret:
openssl rand -hex 32
# Put that secret in CONV_TURN_SECRET in .env, never in the frontend build.
docker compose --env-file .env -f infra/compose.yaml up --build -d
```

Caddy obtains the HTTPS certificate. The signaling service issues TURN HMAC
credentials expiring after one hour. The browser fetches credentials at startup
and on explicit reconnect; ICE repair refreshes configuration. Production errors
redact request URIs, and signaling does not log room tokens, SDP, ICE or document
content. The global per-transport-address join budget is 120/minute; behind a
proxy this budget is shared by that proxy unless a trusted-client-IP policy is
introduced. No forwarded header is trusted by default.

## Verify the deployment

Check `/health`, open the UI over HTTPS, and join from two actual devices on two
networks. Edit on both sides while disconnected, reconnect, and compare text.
Then test `?relay#ROOM_ID` and inspect the visible relay status. Stop signaling
while existing peer channels remain healthy and continue editing. Test background
and resumed tabs, refresh recovery, export and restore. A link alone cannot
recover a room when no reachable participant retains its state.

## Verification status

The Caddy 2.10.2 configuration validates locally, and the Compose/workflow YAML
parses. Direct and forced loopback TURN browser paths have passed. Docker
Desktop's WSL integration is disabled in the current environment, so container
images and this complete Compose deployment have not yet been built or launched
here. No public host/domain or deployment account has been supplied. Publishing
and physical-device/WAN validation remain explicit release gates.

The Sites skill was used for local preview/verification guidance. The deployment
stack here also requires long-running Rust signaling and TURN services, which
must be provisioned on the selected host.
