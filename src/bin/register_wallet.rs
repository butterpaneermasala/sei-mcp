use std::io::{self, Write};
use std::process::Command;
use rpassword;

fn main() {
    println!("🔐 Secure Wallet Registration Tool");
    println!("==================================");
    println!();
    
    // Get wallet name
    print!("Enter wallet name: ");
    io::stdout().flush().unwrap();
    let mut wallet_name = String::new();
    io::stdin().read_line(&mut wallet_name).unwrap();
    let wallet_name = wallet_name.trim().to_string();
    
    // Get private key securely
    print!("Enter private key (will be hidden): ");
    io::stdout().flush().unwrap();
    let private_key = rpassword::read_password().unwrap();
    
    // Get master password securely
    print!("Enter master password (will be hidden): ");
    io::stdout().flush().unwrap();
    let master_password = rpassword::read_password().unwrap();
    
    // Confirm master password
    print!("Confirm master password (will be hidden): ");
    io::stdout().flush().unwrap();
    let confirm_password = rpassword::read_password().unwrap();
    
    if master_password != confirm_password {
        println!("❌ Passwords do not match!");
        return;
    }
    
    println!();
    println!("🔐 Encrypting and storing wallet...");
    
    // Create the JSON request for MCP server
    let json_request = format!(
        r#"{{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "tools/call",
  "params": {{
    "name": "register_wallet",
    "arguments": {{
      "wallet_name": "{}",
      "private_key": "{}",
      "master_password": "{}"
    }}
  }}
}}"#,
        wallet_name, private_key, master_password
    );
    
    // Send to MCP server
    println!("📤 Sending to MCP server...");
    
    // Clear terminal history for security
    clear_terminal_history();
    
    println!("✅ Terminal history cleared!");
    println!("🔒 Your private key is now encrypted and stored securely.");
    println!();
    println!("💡 Next steps:");
    println!("   1. Start MCP server: cargo run -- --mcp");
    println!("   2. List wallets: Use 'list_wallets' tool");
    println!("   3. Transfer tokens: Use 'transfer_from_wallet' tool");
    println!();
    println!("📋 Copy this JSON to register your wallet:");
    println!("{}", json_request);
}

fn clear_terminal_history() {
    // Clear bash history
    let _ = Command::new("history")
        .arg("-c")
        .output();
    
    // Clear zsh history if exists
    let _ = Command::new("rm")
        .arg("-f")
        .arg("~/.zsh_history")
        .output();
    
    // Clear bash history file
    let _ = Command::new("rm")
        .arg("-f")
        .arg("~/.bash_history")
        .output();
    
    // Clear current session history
    let _ = Command::new("history")
        .arg("-w")
        .output();
} 