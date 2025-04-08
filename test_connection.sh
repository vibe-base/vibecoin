#!/bin/bash

echo "Testing connection to seed server 155.138.221.38:30334..."
nc -zv 155.138.221.38 30334

if [ $? -eq 0 ]; then
    echo "Connection successful!"
else
    echo "Connection failed. Please check if the server is running and the port is open."
fi

echo "Starting Vibecoin node with seed server..."
./target/release/vibecoin --config config.toml --genesis genesis.toml --network dev --enable-mining true
