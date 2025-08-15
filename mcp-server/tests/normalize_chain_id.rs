use sei_mcp_server_rs::mcp::handler::normalize_chain_id;

#[test]
fn test_normalize_chain_id_aliases() {
    assert_eq!(normalize_chain_id("sei-testnet"), "sei-evm-testnet");
    assert_eq!(normalize_chain_id("sei-mainnet"), "sei-evm-mainnet");
    assert_eq!(normalize_chain_id("atlantic-2"), "atlantic-2");
}

#[test]
fn test_normalize_chain_id_trimming() {
    assert_eq!(normalize_chain_id("  sei-testnet  "), "sei-evm-testnet");
}
