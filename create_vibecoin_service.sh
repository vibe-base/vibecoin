#!/bin/bash

# Script to create a systemd service for VibeCoin node
# This should be run as root or with sudo

# Check if running as root
if [ "$EUID" -ne 0 ]; then
  echo "Please run as root or with sudo"
  exit 1
fi

echo "Creating VibeCoin systemd service..."

# Create the systemd service file
cat > /etc/systemd/system/vibecoin.service << 'EOF'
[Unit]
Description=VibeCoin Node
After=network.target
Wants=network-online.target
StartLimitIntervalSec=500
StartLimitBurst=5

[Service]
User=root
Group=root
WorkingDirectory=/root/vibecoin
ExecStart=/root/vibecoin/target/release/vibecoin
Restart=on-failure
RestartSec=5s
LimitNOFILE=65536
LimitNPROC=65536

# Make sure the directory exists and has proper permissions
PermissionsStartOnly=true
ExecStartPre=/bin/mkdir -p /root/vibecoin/logs
ExecStartPre=/bin/chown -R root:root /root/vibecoin

# Logging
StandardOutput=append:/root/vibecoin/logs/vibecoin.log
StandardError=append:/root/vibecoin/logs/vibecoin-error.log

# Security
PrivateTmp=true
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
EOF

# Set proper permissions
chmod 644 /etc/systemd/system/vibecoin.service

# Reload systemd to recognize the new service
systemctl daemon-reload

echo "VibeCoin systemd service created successfully!"
echo ""
echo "You can now manage your VibeCoin node with the following commands:"
echo "  Start:   sudo systemctl start vibecoin"
echo "  Stop:    sudo systemctl stop vibecoin"
echo "  Restart: sudo systemctl restart vibecoin"
echo "  Status:  sudo systemctl status vibecoin"
echo "  Enable auto-start: sudo systemctl enable vibecoin"
echo ""
echo "View logs with:"
echo "  sudo journalctl -u vibecoin -f"
echo "  or check the log files in /root/vibecoin/logs/"
