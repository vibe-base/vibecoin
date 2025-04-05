#!/bin/bash

# Script to create a systemd service for VibeCoin node
# This should be run as root or with sudo

# Check if running as root
if [ "$EUID" -ne 0 ]; then
  echo "Please run as root or with sudo"
  exit 1
fi

echo "Creating VibeCoin systemd service..."

# Create the systemd service file - simplified version with journald logging
cat > /etc/systemd/system/vibecoin.service << 'EOF'
[Unit]
Description=VibeCoin Node
After=network.target
Wants=network-online.target

[Service]
Type=simple
User=root
WorkingDirectory=/root/vibecoin
ExecStart=/root/vibecoin/target/release/vibecoin
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

# Reload systemd to recognize the new service
systemctl daemon-reload

echo "VibeCoin systemd service created successfully!"
echo ""
echo "Before starting the service, make sure the binary exists:"
echo "  ls -la /root/vibecoin/target/release/vibecoin"
echo ""
echo "If it doesn't exist, build it with:"
echo "  cd /root/vibecoin && cargo build --release"
echo ""
echo "You can now manage your VibeCoin node with the following commands:"
echo "  Start:   sudo systemctl start vibecoin"
echo "  Stop:    sudo systemctl stop vibecoin"
echo "  Restart: sudo systemctl restart vibecoin"
echo "  Status:  sudo systemctl status vibecoin"
echo "  Enable auto-start: sudo systemctl enable vibecoin"
echo ""
echo "View logs with:"
echo "  tail -f /root/vibecoin/logs/vibecoin.log"
echo "  tail -f /root/vibecoin/logs/vibecoin-error.log"
