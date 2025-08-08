# Security Guide

## 🔐 Where Encrypted Keys Are Stored

### Storage Location
Encrypted private keys are stored in: `~/.sei-mcp-server/wallets.json`

### File Structure
```json
{
  "wallets": {
    "my_wallet": {
      "wallet_name": "my_wallet",
      "encrypted_private_key": "base64_encrypted_key_here",
      "public_address": "0x6ea4dee193ceb368369134b4fda42027081ae1df",
      "created_at": "2024-01-01T00:00:00Z",
      "updated_at": "2024-01-01T00:00:00Z"
    }
  },
  "master_password_hash": "sha256_hash_of_master_password",
  "created_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-01T00:00:00Z"
}
```

### Security Features
- **🔐 AES-256-GCM Encryption**: Each private key is encrypted with a unique nonce
- **🔑 Master Password**: SHA-256 hashed for verification
- **📁 Local Storage**: All data stored locally on your machine
- **🚫 No Plain Text**: Private keys are never stored in plain text

## 🛡️ Secure Wallet Registration

### Option 1: Secure Shell Script (Recommended)
```bash
./register_wallet.sh
```

This tool:
- Hides your private key and password input
- Clears terminal history after use
- Generates the JSON request for you
- Validates password confirmation

### Option 2: Manual Registration
```bash
# Start MCP server
cargo run -- --mcp

# Then send this JSON (replace with your values):
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "register_wallet",
    "arguments": {
      "wallet_name": "my_wallet",
      "private_key": "YOUR_PRIVATE_KEY",
      "master_password": "YOUR_SECURE_PASSWORD"
    }
  }
}
```

## 🔧 MCP Configuration

### Update your `mcp.json`:
```json
{
  "mcpServers": {
    "sei-mcp-server": {
      "command": "cargo",
      "args": [
        "run",
        "--",
        "--mcp"
      ],
      "cwd": "/path/to/sei-mcp-server-rs",
      "env": {
        "CHAIN_RPC_URLS": "{\"sei\":\"https://rpc.sei.io\"}",
        "PORT": "3000"
      }
    }
  }
}
```

**Important**: Replace `/path/to/sei-mcp-server-rs` with your actual project path.

## 🚨 Security Best Practices

### 1. Master Password
- Choose a strong, unique password
- Never share your master password
- If you forget it, you'll need to re-register wallets

### 2. Private Keys
- Never enter private keys in plain text
- Use the secure registration tool
- Clear terminal history after wallet operations

### 3. File Permissions
```bash
# Secure the wallet storage directory
chmod 700 ~/.sei-mcp-server/
chmod 600 ~/.sei-mcp-server/wallets.json
```

### 4. Backup
```bash
# Backup your encrypted wallets
cp ~/.sei-mcp-server/wallets.json ~/backup-wallets.json
```

## 🔍 Troubleshooting

### Wallet Not Found
- Check master password spelling
- Verify wallet was registered successfully
- Check wallet name spelling

### Invalid Master Password
- Double-check your master password
- If forgotten, re-register wallets

### File Permissions
```bash
# Check file permissions
ls -la ~/.sei-mcp-server/
```

### Clear All Data
```bash
# Remove all stored wallets (use with caution!)
rm -rf ~/.sei-mcp-server/
```

## 📋 Quick Setup Checklist

1. **Build the project**
   ```bash
   cargo build --release
   ```

2. **Configure environment**
   ```bash
   # Create .env file
   echo 'CHAIN_RPC_URLS={"sei":"https://rpc.sei.io"}' > .env
   echo 'PORT=3000' >> .env
   ```

3. **Register wallet securely**
   ```bash
   ./register_wallet.sh
   ```

4. **Start MCP server**
   ```bash
   cargo run -- --mcp
   ```

5. **Test wallet operations**
   ```bash
   # List wallets
   echo '{"jsonrpc": "2.0", "id": 1, "method": "tools/call", "params": {"name": "list_wallets", "arguments": {"master_password": "YOUR_PASSWORD"}}}' | cargo run -- --mcp
   ```

## 🔐 Encryption Details

### AES-256-GCM Encryption
- **Algorithm**: AES-256-GCM
- **Key Derivation**: Argon2 from master password
- **Nonce**: 12-byte random nonce per encryption
- **Encoding**: Base64 for storage

### Master Password Hashing
- **Algorithm**: SHA-256
- **Purpose**: Verify master password without storing it
- **Storage**: Hash stored in wallet file

### Confirmation Codes
- **Format**: 3 letters + 3 numbers (shuffled)
- **Expiration**: 5 minutes
- **Purpose**: Prevent accidental transfers 