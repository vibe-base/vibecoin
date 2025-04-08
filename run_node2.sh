#!/bin/bash

echo "Starting second Vibecoin node..."
./target/release/vibecoin --config config_node2.toml --genesis genesis.toml --network dev --enable-mining true
