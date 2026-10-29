FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends coturn && rm -rf /var/lib/apt/lists/*
COPY infra/turn/start.sh /usr/local/bin/start-turn
ENTRYPOINT ["/usr/local/bin/start-turn"]
