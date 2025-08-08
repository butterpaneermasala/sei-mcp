use crate::mcp::wallet_storage::*;

fn main() {
    println!("Testing wallet storage functionality...");
    
    // Test 1: Initialize wallet storage
    match initialize_wallet_storage("test_password") {
        Ok(_) => println!("✅ Wallet storage initialized successfully"),
        Err(e) => println!("❌ Failed to initialize wallet storage: {}", e),
    }
    
    // Test 2: Add a wallet
    match add_wallet_to_storage(
        "test_wallet".to_string(),
        "7f0d4c977cf0b0891798702e6bd740dc2d9aa6195be2365ee947a3c6a08a38fa".to_string(),
        "0x6ea4dee193ceb368369134b4fda42027081ae1df".to_string(),
        "test_password"
    ) {
        Ok(_) => println!("✅ Wallet added successfully"),
        Err(e) => println!("❌ Failed to add wallet: {}", e),
    }
    
    // Test 3: List wallets
    match list_wallets_from_storage() {
        Ok(wallets) => println!("✅ Found {} wallets", wallets.len()),
        Err(e) => println!("❌ Failed to list wallets: {}", e),
    }
    
    // Test 4: Get wallet
    match get_wallet_from_storage("test_wallet", "test_password") {
        Ok(wallet) => println!("✅ Retrieved wallet: {}", wallet.wallet_name),
        Err(e) => println!("❌ Failed to get wallet: {}", e),
    }
    
    println!("Test completed!");
} 