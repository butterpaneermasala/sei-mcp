#!/bin/bash

echo "Testing HTTP API..."
echo "==================="

# Start the HTTP server in the background
echo "Starting HTTP server..."
./target/release/sei-mcp-server-rs &
SERVER_PID=$!

# Wait for server to start
sleep 3

# Test 1: Health check
echo -e "\n1. Testing health endpoint..."
curl -s http://localhost:3000/health

# Test 2: Create wallet
echo -e "\n\n2. Testing create wallet..."
curl -s -X POST http://localhost:3000/wallet/create

# Test 3: Get balance (will fail without valid address, but tests endpoint)
echo -e "\n\n3. Testing balance endpoint..."
curl -s "http://localhost:3000/balance/sei/invalid_address"

# Stop the server
echo -e "\n\nStopping HTTP server..."
kill $SERVER_PID

echo -e "\nHTTP API tests completed!"
