#!/bin/bash

# Manual test script for SEI staking functionality
# Run this script and follow the prompts

echo "🧪 SEI Staking Manual Test"
echo "=========================="
echo ""

# Set environment variables
export PORT=3000
export CHAIN_RPC_URLS="sei-testnet=https://rpc-testnet.sei-apis.com,sei=https://rpc.sei-apis.com"

echo "📋 Configuration:"
echo "  Port: $PORT"
echo "  RPC URLs: $CHAIN_RPC_URLS"
echo ""

echo "🔧 Starting server..."
echo "Press Ctrl+C to stop the server when done testing"
echo ""

# Start server
./target/release/sei-mcp-server-rs
