#!/bin/bash

# Test script for SEI staking functionality on testnet

echo "🚀 Starting SEI MCP Server Test on Testnet"
echo "=========================================="

# Set environment variables for testnet
export PORT=3000
export CHAIN_RPC_URLS="sei-testnet=https://rpc-testnet.sei-apis.com,sei=https://rpc.sei-apis.com"

echo "📋 Configuration:"
echo "  Port: $PORT"
echo "  RPC URLs: $CHAIN_RPC_URLS"
echo ""

# Start the server in background
echo "🔧 Starting MCP server..."
./target/release/sei-mcp-server-rs &
SERVER_PID=$!

# Wait for server to start
sleep 3

echo "✅ Server started with PID: $SERVER_PID"
echo ""

# Test data for staking operations
VALIDATOR_ADDRESS="seivaloper1testvalidatoraddress123456789"
PRIVATE_KEY="1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
STAKE_AMOUNT="1000000"  # 1 SEI in usei
GAS_FEE="7500"

echo "🧪 Testing Staking Functions"
echo "============================"

# Test 1: Get Validators
echo "📊 Test 1: Getting validators..."
curl -X GET "http://localhost:3000/api/staking/sei-testnet/validators" \
  -H "Content-Type: application/json" \
  -w "\nStatus: %{http_code}\nTime: %{time_total}s\n"

echo ""

# Test 2: Get APR
echo "📈 Test 2: Getting staking APR..."
curl -X GET "http://localhost:3000/api/staking/sei-testnet/apr" \
  -H "Content-Type: application/json" \
  -w "\nStatus: %{http_code}\nTime: %{time_total}s\n"

echo ""

# Test 3: Stake Tokens
echo "🔒 Test 3: Staking tokens..."
curl -X POST "http://localhost:3000/api/staking/sei-testnet/stake" \
  -H "Content-Type: application/json" \
  -d "{
    \"validator_address\": \"$VALIDATOR_ADDRESS\",
    \"amount\": \"$STAKE_AMOUNT\",
    \"private_key\": \"$PRIVATE_KEY\",
    \"gas_fee\": $GAS_FEE
  }" \
  -w "\nStatus: %{http_code}\nTime: %{time_total}s\n"

echo ""

# Test 4: Unstake Tokens
echo "🔓 Test 4: Unstaking tokens..."
curl -X POST "http://localhost:3000/api/staking/sei-testnet/unstake" \
  -H "Content-Type: application/json" \
  -d "{
    \"validator_address\": \"$VALIDATOR_ADDRESS\",
    \"amount\": \"$STAKE_AMOUNT\",
    \"private_key\": \"$PRIVATE_KEY\",
    \"gas_fee\": $GAS_FEE
  }" \
  -w "\nStatus: %{http_code}\nTime: %{time_total}s\n"

echo ""

# Test 5: Claim Rewards
echo "💰 Test 5: Claiming rewards..."
curl -X POST "http://localhost:3000/api/staking/sei-testnet/claim-rewards" \
  -H "Content-Type: application/json" \
  -d "{
    \"validator_address\": \"$VALIDATOR_ADDRESS\",
    \"private_key\": \"$PRIVATE_KEY\",
    \"gas_fee\": $GAS_FEE
  }" \
  -w "\nStatus: %{http_code}\nTime: %{time_total}s\n"

echo ""

# Test 6: MCP Protocol Test
echo "🤖 Test 6: Testing MCP Protocol..."
echo '{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "test-client", "version": "1.0.0"}}}' | nc localhost 3000

echo ""

echo "🧹 Cleaning up..."
kill $SERVER_PID
echo "✅ Server stopped"
echo ""
echo "🎉 Test completed!"
