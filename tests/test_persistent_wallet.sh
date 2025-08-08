#!/bin/bash

echo "Testing Persistent Wallet Storage (like Foundry Cast)"

# Test 1: Register a wallet
echo "1. Registering wallet 'my_wallet'..."
echo '{"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "register_wallet", "arguments": {"wallet_name": "my_wallet", "private_key": "7f0d4c977cf0b0891798702e6bd740dc2d9aa6195be2365ee947a3c6a08a38fa", "master_password": "my_secure_password"}}}' | cargo run -- --mcp

echo -e "\n2. Checking if wallet file was created..."
ls -la ~/.sei-mcp-server/

echo -e "\n3. Listing wallets..."
echo '{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "list_wallets", "arguments": {"master_password": "my_secure_password"}}}' | cargo run -- --mcp

echo -e "\n4. Testing wallet persistence by restarting server..."
echo '{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "list_wallets", "arguments": {"master_password": "my_secure_password"}}}' | cargo run -- --mcp

echo -e "\n5. Testing transfer initiation..."
echo '{"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "transfer_from_wallet", "arguments": {"wallet_name": "my_wallet", "to_address": "0x1234567890123456789012345678901234567890", "amount": "1000000000000000000", "chain_id": "sei", "master_password": "my_secure_password"}}}' | cargo run -- --mcp

echo -e "\nTest completed!" 