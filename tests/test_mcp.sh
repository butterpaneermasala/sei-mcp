#!/bin/bash

# Test script for MCP server with encryption and confirmation

echo "Testing MCP server with encryption and confirmation..."

# Start the MCP server in the background
cargo run -- --mcp &
SERVER_PID=$!

# Wait for server to start
sleep 3

echo "1. Registering wallet..."
echo '{"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "register_wallet", "arguments": {"wallet_name": "test_wallet", "private_key": "7f0d4c977cf0b0891798702e6bd740dc2d9aa6195be2365ee947a3c6a08a38fa", "master_password": "my_secure_password"}}}' | nc localhost 3000

echo -e "\n2. Testing transfer initiation..."
TRANSFER_RESPONSE=$(echo '{"jsonrpc": "2.0", "id": 2, "method": "tools/call", "params": {"name": "transfer_from_wallet", "arguments": {"wallet_name": "test_wallet", "to_address": "0x1234567890123456789012345678901234567890", "amount": "1000000000000000000", "chain_id": "sei"}}}' | nc localhost 3000)

echo "Transfer response: $TRANSFER_RESPONSE"

# Extract confirmation code from response (this is a simplified extraction)
CONFIRMATION_CODE=$(echo "$TRANSFER_RESPONSE" | grep -o '"confirmation_code":"[^"]*"' | cut -d'"' -f4)
TRANSACTION_ID=$(echo "$TRANSFER_RESPONSE" | grep -o '"transaction_id":"[^"]*"' | cut -d'"' -f4)

if [ ! -z "$CONFIRMATION_CODE" ] && [ ! -z "$TRANSACTION_ID" ]; then
    echo -e "\n3. Confirming transaction with code: $CONFIRMATION_CODE"
    echo "{\"jsonrpc\": \"2.0\", \"id\": 3, \"method\": \"tools/call\", \"params\": {\"name\": \"confirm_transaction\", \"arguments\": {\"transaction_id\": \"$TRANSACTION_ID\", \"confirmation_code\": \"$CONFIRMATION_CODE\"}}}" | nc localhost 3000
else
    echo "Failed to extract confirmation code or transaction ID"
fi

# Clean up
kill $SERVER_PID
echo -e "\nTest completed." 