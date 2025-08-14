#!/bin/bash
# Initialize
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}'
# Initialized notification (no response expected)
echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
# List tools
echo '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
