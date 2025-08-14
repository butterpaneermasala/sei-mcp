#!/bin/bash

# This script runs all tests for the sei-mcp-server-rs project,
# including Rust integration tests and shell-based MCP server tests.

# Exit immediately if a command exits with a non-zero status.
set -e

echo "🚀 Starting Full Test Suite..."
echo "=============================="

# 1. Build the project in release mode to ensure binaries are up to date
echo "Building project in release mode..."
cargo build --release
echo "✅ Build complete."
echo "---------------------------------"

# 2. Run Rust integration tests
echo "Running Rust integration tests (cargo test)..."
cargo test --release
echo "✅ Rust integration tests passed."
echo "---------------------------------"

# 3. Run the basic MCP server test script
echo "Running basic MCP server test (tests/test_mcp_server.sh)..."
bash tests/test_mcp_server.sh
echo "✅ Basic MCP server test passed."
echo "---------------------------------"

# 4. Run the persistent wallet end-to-end test script
echo "Running persistent wallet E2E test (tests/test_persistent_wallet.sh)..."
bash tests/test_persistent_wallet.sh
echo "✅ Persistent wallet E2E test passed."
echo "---------------------------------"

echo "🎉🎉🎉 All tests passed successfully! 🎉🎉🎉"
