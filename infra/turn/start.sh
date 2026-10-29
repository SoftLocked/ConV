#!/bin/sh
set -eu
exec turnserver --no-cli --no-tls --no-dtls --fingerprint --use-auth-secret \
  --static-auth-secret="$CONV_TURN_SECRET" --realm="$CONV_HOST" \
  --external-ip="$CONV_PUBLIC_IP" --min-port=49160 --max-port=49200 \
  --no-multicast-peers --log-file=stdout
