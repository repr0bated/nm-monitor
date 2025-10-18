# Multi-stage build for ovs-port-agent
FROM rust:1.75-slim as builder

# Install system dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app user
RUN useradd -r -s /bin/false ovs-agent

# Set working directory
WORKDIR /app

# Copy dependency manifests
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src/
COPY nm-monitor ./nm-monitor/

# Build the application in release mode
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    openvswitch-switch \
    systemd \
    dbus \
    iproute2 \
    net-tools \
    procps \
    && rm -rf /var/lib/apt/lists/*

# Create ovs-agent user
RUN useradd -r -s /bin/false ovs-agent

# Create necessary directories
RUN mkdir -p /etc/ovs-port-agent /var/lib/ovs-port-agent

# Copy binary from builder stage
COPY --from=builder /app/target/release/ovs-port-agent /usr/local/bin/
COPY --from=builder /app/target/release/ovsdb-dbus-wrapper /usr/local/bin/
COPY --from=builder /app/target/release/ovsdb-fuse-mount /usr/local/bin/
COPY --from=builder /app/target/release/system-introspection /usr/local/bin/

# Copy systemd service files
COPY systemd/ /etc/systemd/system/

# Copy default configuration
COPY config/config.toml.example /etc/ovs-port-agent/config.toml

# Set proper ownership
RUN chown -R ovs-agent:ovs-agent /etc/ovs-port-agent /var/lib/ovs-port-agent

# Enable services
RUN systemctl enable ovs-port-agent.service ovsdb-dbus-wrapper.service

# Create entrypoint script
COPY <<EOF /usr/local/bin/entrypoint.sh
#!/bin/bash
set -e

# Wait for D-Bus
while ! pgrep dbus-daemon > /dev/null; do
    echo "Waiting for D-Bus..."
    sleep 1
done

# Start services
exec systemd
EOF

RUN chmod +x /usr/local/bin/entrypoint.sh

# Expose any necessary ports (adjust as needed)
EXPOSE 8080

# Set user
USER ovs-agent

# Set working directory
WORKDIR /var/lib/ovs-port-agent

# Use systemd as init system
ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
