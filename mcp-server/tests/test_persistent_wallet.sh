#!/bin/bash

# This script provides an end-to-end test for the persistent wallet functionality.
# It covers registration, listing, balance checking, and removal of a wallet.

echo "🧪 Starting Persistent Wallet E2E Test..."

# Ensure a clean state by removing any previous wallet storage
rm -f ~/.sei-mcp-server/wallets.json

# Define variables
MASTER_PASS="my_super_secret_password_123"
WALLET_NAME="persistent_test_wallet"
# A sample private key for testing (DO NOT USE REAL FUNDS)
PRIVATE_KEY="0x7f0d4c977cf0b0891798702e6bd740dc2d9aa6195be2365ee947a3c6a08a38fa"
# The corresponding public address for the private key above
PUBLIC_ADDRESS="0x9e354f472F2F26A5925344413eD33256222b1A3C"
CHAIN_ID="sei-testnet"

# Function to send a JSON-RPC request via cargo run
send_mcp_request() {
    echo "$1" | cargo run --release -- --mcp
}

# 1. Register a new wallet
echo "1. Registering wallet '$WALLET_NAME'..."
REG_REQUEST=$(cat <<EOF
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "register_wallet",
    "arguments": {
      "wallet_name": "$WALLET_NAME",
      "private_key": "$PRIVATE_KEY",
      "master_password": "$MASTER_PASS"
    }
  }
}
EOF
)
REG_RESPONSE=$(send_mcp_request "$REG_REQUEST")
echo "$REG_RESPONSE"

if ! echo "$REG_RESPONSE" | grep -q "Wallet '$WALLET_NAME' registered successfully."; then
    echo "❌ Test Failed: Wallet registration failed."
    exit 1
fi
echo "✅ Wallet registered."
echo "---------------------------------"

# 2. Verify the wallet file was created
echo "2. Checking for wallet file..."
if [ -f ~/.sei-mcp-server/wallets.json ]; then
    echo "✅ Wallet file found at ~/.sei-mcp-server/wallets.json"
else
    echo "❌ Test Failed: Wallet file was not created."
    exit 1
fi
echo "---------------------------------"

# 3. List wallets to confirm registration
echo "3. Listing wallets..."
LIST_REQUEST=$(cat <<EOF
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "list_wallets",
    "arguments": { "master_password": "$MASTER_PASS" }
  }
}
EOF
)
LIST_RESPONSE=$(send_mcp_request "$LIST_REQUEST")
echo "$LIST_RESPONSE"

if ! echo "$LIST_RESPONSE" | grep -q "$WALLET_NAME"; then
    echo "❌ Test Failed: Registered wallet not found in the list."
    exit 1
fi
echo "✅ Wallet found in list."
echo "---------------------------------"

# 4. Get the balance of the registered wallet
echo "4. Getting balance for '$WALLET_NAME'..."
BALANCE_REQUEST=$(cat <<EOF
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "get_wallet_balance",
    "arguments": {
      "wallet_name": "$WALLET_NAME",
      "chain_id": "$CHAIN_ID",
      "master_password": "$MASTER_PASS"
    }
  }
}
EOF
)
BALANCE_RESPONSE=$(send_mcp_request "$BALANCE_REQUEST")
echo "$BALANCE_RESPONSE"

if ! echo "$BALANCE_RESPONSE" | grep -q "Balance for $WALLET_NAME"; then
    echo "❌ Test Failed: Could not get balance for the wallet."
    exit 1
fi
echo "✅ Balance check successful."
echo "---------------------------------"

# 5. Remove the wallet
echo "5. Removing wallet '$WALLET_NAME'..."
REMOVE_REQUEST=$(cat <<EOF
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "name": "remove_wallet",
    "arguments": {
      "wallet_name": "$WALLET_NAME",
      "master_password": "$MASTER_PASS"
    }
  }
}
EOF
)
REMOVE_RESPONSE=$(send_mcp_request "$REMOVE_REQUEST")
echo "$REMOVE_RESPONSE"

if ! echo "$REMOVE_RESPONSE" | grep -q "Wallet '$WALLET_NAME' removed."; then
    echo "❌ Test Failed: Wallet removal failed."
    exit 1
fi
echo "✅ Wallet removed."
echo "---------------------------------"

# 6. List wallets again to confirm removal
echo "6. Listing wallets again to confirm removal..."
LIST_AGAIN_RESPONSE=$(send_mcp_request "$LIST_REQUEST")
echo "$LIST_AGAIN_RESPONSE"

if echo "$LIST_AGAIN_RESPONSE" | grep -q "$WALLET_NAME"; then
    echo "❌ Test Failed: Wallet was not actually removed from storage."
    exit 1
fi
echo "✅ Wallet no longer in list."
echo "---------------------------------"

echo "🎉 All persistent wallet tests passed!"
exit 0
