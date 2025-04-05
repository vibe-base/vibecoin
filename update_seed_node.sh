#!/bin/bash

# Script to update a VibeCoin seed node
# This should be run as root or with sudo

# Check if running as root
if [ "$EUID" -ne 0 ]; then
  echo "Please run as root or with sudo"
  exit 1
fi

echo "Updating VibeCoin seed node..."

# Stop the current service
systemctl stop vibecoin

# Update the service file
cat > /etc/systemd/system/vibecoin.service << 'EOF'
[Unit]
Description=VibeCoin Node
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=root
WorkingDirectory=/root/vibecoin
ExecStart=/root/vibecoin/target/release/vibecoin --enable-networking --p2p-listen-addr 0.0.0.0:8333
Restart=on-failure
RestartSec=5s

# Let systemd handle the logging
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
EOF

# Set proper permissions
chmod 644 /etc/systemd/system/vibecoin.service

# Pull the latest code
cd /root/vibecoin
git pull

# Rebuild the project
cargo build --release

# Reload systemd and start the service
systemctl daemon-reload
systemctl start vibecoin

# Check if it's listening on port 8333
netstat -tuln | grep 8333

echo "VibeCoin seed node updated successfully!"
echo ""
echo "You can check the status with:"
echo "  systemctl status vibecoin"
echo ""
echo "View logs with:"
echo "  journalctl -u vibecoin -f"
