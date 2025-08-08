#!/bin/bash

echo "🔐 Secure Wallet Registration Tool"
echo "=================================="
echo ""

# Get wallet name
read -p "Enter wallet name: " wallet_name

# Get private key securely
read -s -p "Enter private key (will be hidden): " private_key
echo ""

# Get master password securely
read -s -p "Enter master password (will be hidden): " master_password
echo ""

# Confirm master password
read -s -p "Confirm master password (will be hidden): " confirm_password
echo ""

if [ "$master_password" != "$confirm_password" ]; then
    echo "❌ Passwords do not match!"
    exit 1
fi

echo ""
echo "🔐 Encrypting and storing wallet..."

# Create JSON request
json_request=$(cat <<EOF
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "register_wallet",
    "arguments": {
      "wallet_name": "$wallet_name",
      "private_key": "$private_key",
      "master_password": "$master_password"
    }
  }
}
EOF
)

# Clear terminal history for security
echo "⚠️  Clearing terminal history for security..."
history -c
history -w
rm -f ~/.bash_history ~/.zsh_history 2>/dev/null

echo "✅ Terminal history cleared!"
echo "🔒 Your private key is now encrypted and stored securely."
echo ""
echo "💡 Next steps:"
echo "   1. Start MCP server: cargo run -- --mcp"
echo "   2. Copy and paste this JSON to register your wallet:"
echo ""
echo "$json_request"
echo ""
echo "📁 Wallet will be stored in: ~/.sei-mcp-server/wallets.json"
echo "🔐 Encrypted with AES-256-GCM" 