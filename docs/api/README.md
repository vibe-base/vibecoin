# Vibecoin API Documentation

This directory contains documentation for the Vibecoin APIs.

## API Overview

Vibecoin provides several APIs for interacting with the blockchain:

- **JSON-RPC API**: Primary API for interacting with the Vibecoin network
- **WebSocket API**: For real-time updates and subscriptions
- **REST API**: For simpler HTTP-based interactions

## API Categories

- [Node API](node.md): APIs for interacting with Vibecoin nodes
- [Wallet API](wallet.md): APIs for wallet management
- [Transaction API](transaction.md): APIs for creating and managing transactions
- [Smart Contract API](smart_contract.md): APIs for deploying and interacting with smart contracts
- [Block API](block.md): APIs for querying blocks and blockchain data

## Authentication

Most API endpoints require authentication using API keys or JWT tokens. See the [Authentication Guide](authentication.md) for details.

## Rate Limiting

API requests are subject to rate limiting to prevent abuse. See the [Rate Limiting Guide](rate_limiting.md) for details.

## Error Handling

All APIs use standard error codes and formats. See the [Error Handling Guide](error_handling.md) for details.

## Versioning

APIs are versioned to ensure backward compatibility. The current version is v1.
