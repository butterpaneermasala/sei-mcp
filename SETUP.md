# Quick Setup Guide

## 1. Build the Project

```bash
cargo build --release
```

## 2. Configure Environment

Create a `.env` file:

```env
CHAIN_RPC_URLS={"sei":"https://rpc.sei.io"}
PORT=3000
```

## 3. Start MCP Server

```bash
cargo run -- --mcp
```

## 4. Register Your First Wallet

Send this JSON to the MCP server:

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {
    "name": "register_wallet",
    "arguments": {
      "wallet_name": "my_wallet",
      "private_key": "YOUR_PRIVATE_KEY_HERE",
      "master_password": "YOUR_SECURE_PASSWORD"
    }
  }
}
```

## 5. List Your Wallets

```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "tools/call",
  "params": {
    "name": "list_wallets",
    "arguments": {
      "master_password": "YOUR_SECURE_PASSWORD"
    }
  }
}
```

## 6. Transfer Tokens

### Step 1: Initiate Transfer
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "tools/call",
  "params": {
    "name": "transfer_from_wallet",
    "arguments": {
      "wallet_name": "my_wallet",
      "to_address": "RECIPIENT_ADDRESS",
      "amount": "1000000000000000000",
      "chain_id": "sei",
      "master_password": "YOUR_SECURE_PASSWORD"
    }
  }
}
```

### Step 2: Confirm Transfer
Use the confirmation code from Step 1:

```json
{
  "jsonrpc": "2.0",
  "id": 4,
  "method": "tools/call",
  "params": {
    "name": "confirm_transaction",
    "arguments": {
      "transaction_id": "FROM_STEP_1",
      "confirmation_code": "FROM_STEP_1",
      "master_password": "YOUR_SECURE_PASSWORD"
    }
  }
}
```

## Key Differences from Previous Version

1. **🔐 Encrypted Storage**: Private keys are now encrypted with AES-256-GCM
2. **💾 Persistence**: Wallets survive server restarts
3. **🔑 Master Password**: All operations require your master password
4. **✅ Confirmation**: Transfers require a two-step confirmation process
5. **📁 Local Storage**: Wallets stored in `~/.sei-mcp-server/wallets.json`

## Security Notes

- **Master Password**: Choose a strong password - it protects all your wallets
- **Confirmation Codes**: 6-character codes expire after 5 minutes
- **Private Keys**: Never stored in plain text, always encrypted
- **Local Storage**: All data stored locally on your machine

## Troubleshooting

### Wallet Not Found
- Ensure you're using the correct master password
- Check that the wallet was registered successfully
- Verify the wallet name spelling

### Invalid Master Password
- Double-check your master password
- If you forgot it, you'll need to re-register wallets

### Transaction Expired
- Confirmation codes expire after 5 minutes
- Initiate a new transfer if expired

## Testing

Run the test suite:

```bash
# Run all tests
cargo test

# Run integration tests
./tests/test_persistent_wallet.sh
``` 