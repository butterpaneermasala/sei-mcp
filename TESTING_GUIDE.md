# 🧪 SEI Staking Testing Guide

This guide will help you test the staking functionality on SEI testnet.

## 🚀 Quick Start

### Option 1: Automated Test
```bash
./test_staking.sh
```

### Option 2: Manual Testing
```bash
./manual_test.sh
```

## 📋 Prerequisites

1. **Build the project**:
   ```bash
   cargo build --release
   ```

2. **Set environment variables**:
   ```bash
   export PORT=3000
   export CHAIN_RPC_URLS="sei-testnet=https://rpc-testnet.sei-apis.com,sei=https://rpc.sei-apis.com"
   ```

## 🧪 Manual Testing Steps

### Step 1: Start the Server
```bash
./target/release/sei-mcp-server-rs
```

### Step 2: Test Validator Information
```bash
# Get all validators
curl -X GET "http://localhost:3000/api/staking/sei-testnet/validators" \
  -H "Content-Type: application/json"

# Get staking APR
curl -X GET "http://localhost:3000/api/staking/sei-testnet/apr" \
  -H "Content-Type: application/json"
```

### Step 3: Test Staking Operations

#### Test Staking (Delegate)
```bash
curl -X POST "http://localhost:3000/api/staking/sei-testnet/stake" \
  -H "Content-Type: application/json" \
  -d '{
    "validator_address": "seivaloper1testvalidatoraddress123456789",
    "amount": "1000000",
    "private_key": "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
    "gas_fee": 7500
  }'
```

#### Test Unstaking (Undelegate)
```bash
curl -X POST "http://localhost:3000/api/staking/sei-testnet/unstake" \
  -H "Content-Type: application/json" \
  -d '{
    "validator_address": "seivaloper1testvalidatoraddress123456789",
    "amount": "1000000",
    "private_key": "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
    "gas_fee": 7500
  }'
```

#### Test Claiming Rewards
```bash
curl -X POST "http://localhost:3000/api/staking/sei-testnet/claim-rewards" \
  -H "Content-Type: application/json" \
  -d '{
    "validator_address": "seivaloper1testvalidatoraddress123456789",
    "private_key": "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
    "gas_fee": 7500
  }'
```

## 🤖 MCP Protocol Testing

### Test MCP Initialization
```bash
echo '{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2024-11-05", "capabilities": {}, "clientInfo": {"name": "test-client", "version": "1.0.0"}}}' | nc localhost 3000
```

### Test MCP Tools List
```bash
echo '{"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}' | nc localhost 3000
```

### Test MCP Staking Tools
```bash
# Test stake_tokens tool
echo '{"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "stake_tokens", "arguments": {"chain_id": "sei-testnet", "validator_address": "seivaloper1testvalidatoraddress123456789", "amount": "1000000", "private_key": "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef", "gas_fee": 7500}}}' | nc localhost 3000

# Test unstake_tokens tool
echo '{"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "unstake_tokens", "arguments": {"chain_id": "sei-testnet", "validator_address": "seivaloper1testvalidatoraddress123456789", "amount": "1000000", "private_key": "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef", "gas_fee": 7500}}}' | nc localhost 3000

# Test claim_rewards tool
echo '{"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "claim_rewards", "arguments": {"chain_id": "sei-testnet", "validator_address": "seivaloper1testvalidatoraddress123456789", "private_key": "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef", "gas_fee": 7500}}}' | nc localhost 3000

# Test get_validators tool
echo '{"jsonrpc": "2.0", "id": 6, "method": "tools/call", "params": {"name": "get_validators", "arguments": {"chain_id": "sei-testnet"}}}' | nc localhost 3000

# Test get_staking_apr tool
echo '{"jsonrpc": "2.0", "id": 7, "method": "tools/call", "params": {"name": "get_staking_apr", "arguments": {"chain_id": "sei-testnet"}}}' | nc localhost 3000
```

## 🔍 Expected Results

### ✅ Successful Responses
- **Validators**: Should return a list of validators with commission rates
- **APR**: Should return the current staking APR
- **Staking Operations**: Should return placeholder transaction hashes with validation

### ❌ Error Cases to Test
- Invalid validator address (not starting with "seivaloper")
- Amount too small (less than 1,000,000 usei)
- Invalid private key format
- Unsupported chain_id

## 🐛 Debugging

### Check Server Logs
The server will output detailed logs showing:
- Input validation results
- Network parameter resolution
- Transaction processing steps
- Error details

### Common Issues
1. **Port already in use**: Change PORT environment variable
2. **Network connectivity**: Check if RPC endpoints are accessible
3. **Validation errors**: Check input format requirements

## 📊 Test Data

### Valid Test Data
- **Validator Address**: `seivaloper1testvalidatoraddress123456789`
- **Amount**: `1000000` (1 SEI in usei)
- **Private Key**: `1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef`
- **Gas Fee**: `7500`

### Invalid Test Data (for error testing)
- **Invalid Validator**: `sei1invalidaddress`
- **Too Small Amount**: `100000` (0.1 SEI)
- **Invalid Private Key**: `invalid_key`

## 🎯 Testing Checklist

- [ ] Server starts successfully
- [ ] Validators endpoint returns data
- [ ] APR endpoint returns data
- [ ] Staking operation validates inputs
- [ ] Unstaking operation validates inputs
- [ ] Claim rewards operation validates inputs
- [ ] Error handling works for invalid inputs
- [ ] MCP protocol responds correctly
- [ ] All MCP tools are available

## 🚀 Production Testing

For production testing, you would need:
1. Real SEI testnet private keys
2. Actual validator addresses from the testnet
3. Real SEI tokens for staking
4. Implementation of actual transaction signing

The current implementation provides full validation and placeholder responses, ready for the final transaction signing implementation.
