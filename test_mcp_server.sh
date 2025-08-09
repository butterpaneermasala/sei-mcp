#!/bin/bash

echo "Testing MCP Server..."
echo "======================"

# Test 1: Initialize
echo -e "\n1. Testing initialize..."
echo '{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}' | ./target/release/sei-mcp-server-rs --mcp

# Test 2: List tools
echo -e "\n2. Testing tools/list..."
echo '{"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}' | ./target/release/sei-mcp-server-rs --mcp

# Test 3: Health check
echo -e "\n3. Testing health..."
echo '{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "health", "arguments": {}}}' | ./target/release/sei-mcp-server-rs --mcp

echo -e "\nMCP Server tests completed!"
