# Sei MCP Server

A Model Context Protocol (MCP) server for Sei blockchain operations with persistent encrypted wallet storage, plus an external Faucet API integration.

## Features

- 🔐 **AES-256-GCM Encrypted Wallet Storage** - Private keys are encrypted and stored locally
- 💾 **Persistent Storage** - Wallets survive server restarts
- 🔑 **Master Password Protection** - Single password protects all wallets
- ✅ **Two-Step Transfer Confirmation** - Secure transfer workflow with confirmation codes
- 🛠️ **MCP Protocol Support** - Standard MCP tools for blockchain operations
- 🌐 **HTTP API Support** - Traditional REST API endpoints

## Installation

```bash
# Clone the repository
git clone <repository-url>
cd sei-mcp-server-rs

# Install dependencies
cargo build --release
```

## Configuration

Create a `.env` file in the project root:

```env
# REQUIRED: JSON map of chain_id -> RPC URL
# Example contains both EVM and native testnets
CHAIN_RPC_URLS={"sei-evm-testnet":"https://evm-rpc-testnet.sei-apis.com","sei-native-testnet":"https://rpc-testnet.sei-apis.com"}

# Server port (HTTP mode)
PORT=3000

# External Faucet API base URL
FAUCET_API_URL=https://sei-mcp.onrender.com

# Optional (only if you use direct-signed /api/tx/send):
# EVM default sender key (back-compat fallbacks: FAUCET_PRIVATE_KEY_EVM, FAUCET_PRIVATE_KEY)
TX_PRIVATE_KEY_EVM=0x...
# Optional default sender address for native sends (back-compat fallback: FAUCET_ADDRESS)
DEFAULT_SENDER_ADDRESS=sei1...
# Native send parameters (back-compat fallbacks supported)
NATIVE_DENOM=usei
NATIVE_GAS_LIMIT=200000
NATIVE_FEE_AMOUNT=5000
NATIVE_CHAIN_ID=atlantic-2
NATIVE_BECH32_HRP=sei
```

Notes:
- CHAIN_RPC_URLS is now required. There is no localhost fallback.
- The Faucet is handled entirely by the external API. MCP does not store faucet keys.

### MCP Client Configuration

Update your `mcp.json` file:

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
        "CHAIN_RPC_URLS": "{\"sei-evm-testnet\":\"https://evm-rpc-testnet.sei-apis.com\",\"sei-native-testnet\":\"https://rpc-testnet.sei-apis.com\"}",
        "FAUCET_API_URL": "https://sei-mcp.onrender.com",
        "PORT": "3000"
      }
    }
  }
}
```

**Important**: Replace `/path/to/sei-mcp-server-rs` with the actual path to your project directory.

## Usage

### MCP Server Mode (Recommended)

The MCP server runs on stdin/stdout and provides encrypted wallet storage:

```bash
# Start MCP server
cargo run -- --mcp
```

### HTTP Server Mode

```bash
# Start HTTP server
cargo run
```

## Secure Wallet Registration

For maximum security, use the provided secure registration tool:

```bash
# Run the secure wallet registration tool
./register_wallet.sh
```

This tool:
- 🔐 Hides your private key and password input
- 🧹 Clears terminal history after use
- 📋 Generates the JSON request for you to copy/paste
- ✅ Validates password confirmation

## Wallet Management

### 1. Register a Wallet

First, register a wallet with encryption:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "register_wallet",
    "arguments": {
      "wallet_name": "my_wallet",
      "private_key": "0x7f0d4c977cf0b0891798702e6bd740dc2d9aa6195be2365ee947a3c6a08a38fa",
      "master_password": "your_secure_password"
    }
  }
}
```

### 2. List Stored Wallets

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "list_wallets",
    "arguments": {
      "master_password": "your_secure_password"
    }
  }
}
```

### 3. Get Wallet Balance

```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "get_wallet_balance",
    "arguments": {
      "wallet_name": "my_wallet",
      "chain_id": "sei",
      "master_password": "your_secure_password"
    }
  }
}
```

### 4. Transfer Tokens (Two-Step Process)

#### Step 1: Initiate Transfer

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "name": "transfer_from_wallet",
    "arguments": {
      "wallet_name": "my_wallet",
      "to_address": "0x1234567890123456789012345678901234567890",
      "amount": "1000000000000000000",
      "chain_id": "sei",
      "master_password": "your_secure_password"
    }
  }
}
```

This returns a confirmation code and transaction ID.

#### Step 2: Confirm Transfer

```json
{
  "jsonrpc": "2.0",
  "id": 5,
  "method": "tools/call",
  "params": {
    "name": "confirm_transaction",
    "arguments": {
      "transaction_id": "ABC123",
      "confirmation_code": "XYZ789",
      "master_password": "your_secure_password"
    }
  }
}
```

### 5. Remove Wallet

```json
{
  "jsonrpc": "2.0",
  "id": 6,
  "method": "tools/call",
  "params": {
    "name": "remove_wallet",
    "arguments": {
      "wallet_name": "my_wallet",
      "master_password": "your_secure_password"
    }
  }
}
```

## Available Tools

### Basic Tools
- `get_balance` - Get address balance
- `create_wallet` - Create new wallet
- `import_wallet` - Import wallet from private key/mnemonic
- `get_transaction_history` - Get transaction history
- `estimate_fees` - Estimate transaction fees
- `transfer_sei` - Direct transfer (requires private key)
- `request_faucet` - Requests tokens via the external Faucet API (enforces cooldowns and rate-limits)

### Enhanced Tools (with Persistent Storage)
- `register_wallet` - Register wallet with encryption
- `list_wallets` - List all stored wallets
- `get_wallet_balance` - Get balance of stored wallet
- `transfer_from_wallet` - Transfer from stored wallet (two-step)
- `confirm_transaction` - Confirm pending transaction
- `remove_wallet` - Remove wallet from storage

## Security Features

### 🔐 Encryption
- Private keys encrypted with AES-256-GCM
- Argon2 key derivation from master password
- Unique nonce for each encryption
- Base64 encoding for storage

### 🔑 Master Password
- SHA-256 hashed for verification
- Required for all wallet operations
- Protects all stored wallets

### ✅ Confirmation System
- 6-character confirmation codes (3 letters + 3 numbers)
- 5-minute expiration for pending transactions
- Two-step transfer process

## Storage Location

Wallets are stored in: `~/.sei-mcp-server/wallets.json`

The file structure:
```json
{
  "wallets": {
    "wallet_name": {
      "wallet_name": "my_wallet",
      "encrypted_private_key": "base64_encrypted_key",
      "public_address": "0x...",
      "created_at": "2024-01-01T00:00:00Z",
      "updated_at": "2024-01-01T00:00:00Z"
    }
  },
  "master_password_hash": "sha256_hash",
  "created_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-01T00:00:00Z"
}
```

## Testing

Run the test suite:

```bash
# Run all tests
cargo test

# Run specific test
cargo test test_wallet_storage

# Run integration tests
./tests/test_persistent_wallet.sh
```

## Development

### Project Structure
```
src/
├── mcp/
│   ├── encryption.rs      # AES-256-GCM encryption
│   ├── wallet_storage.rs  # Persistent storage management
│   ├── enhanced_tools.rs  # MCP tools with storage
│   ├── tools.rs          # Basic MCP tools
│   ├── protocol.rs       # MCP protocol definitions
│   └── transport.rs      # MCP transport layer
├── blockchain/           # Blockchain client and services
├── api/                 # HTTP API endpoints
└── main.rs              # Application entry point
```

### Adding New Tools

1. Add tool function in `src/mcp/enhanced_tools.rs`
2. Add tool definition in `list_enhanced_tools()`
3. Add dispatch case in `src/mcp/mod.rs`

## License

MIT License 