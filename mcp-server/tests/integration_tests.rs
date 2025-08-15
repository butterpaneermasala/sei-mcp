// // tests/integration_tests.rs

// use sei_mcp_server_rs::{
//     config::Config,
//     mcp::wallet_storage::{
//         add_wallet_to_storage, get_decrypted_private_key_from_storage,
//         initialize_wallet_storage, list_wallets_from_storage, remove_wallet_from_storage,
//         WalletStorage,
//     },
//     blockchain::services::wallet::import_wallet,
// };
// use std::fs;
// use tempfile::tempdir;

// /// Helper function to set up a temporary wallet storage for testing.
// fn setup_test_storage(password: &str) -> tempfile::TempDir {
//     let dir = tempdir().expect("Failed to create temp dir");
//     let storage_path = dir.path().join("wallets.json");

//     // Override the default path for the duration of the test.
//     std::env::set_var("WALLET_STORAGE_PATH", storage_path.to_str().unwrap());

//     // Initialize storage
//     let mut storage = WalletStorage::new(password).expect("Failed to create new wallet storage");
//     storage.save_to_file(&storage_path).expect("Failed to save initial empty storage");

//     dir
// }

// #[test]
// fn test_wallet_storage_initialization_and_verification() {
//     let dir = setup_test_storage("test_password");
//     let storage_path = dir.path().join("wallets.json");

//     // Load from file and verify password
//     let loaded_storage = WalletStorage::load_from_file(&storage_path, "test_password")
//         .expect("Failed to load storage with correct password");
//     assert!(loaded_storage.verify_master_password("test_password"));
//     assert!(!loaded_storage.verify_master_password("wrong_password"));
// }

// #[tokio::test]
// async fn test_add_and_retrieve_wallet() {
//     let _dir = setup_test_storage("master_pass");
//     initialize_wallet_storage("master_pass").expect("Storage initialization failed");

//     let pk = "0x7f0d4c977cf0b0891798702e6bd740dc2d9aa6195be2365ee947a3c6a08a38fa";
//     let wallet_info = import_wallet(pk).expect("Failed to import wallet");
//     let address = wallet_info.address.clone();

//     // Add a wallet
//     add_wallet_to_storage("test_wallet_1".to_string(), pk.to_string(), address, "master_pass")
//         .expect("Failed to add wallet");

//     // List wallets
//     let wallets = list_wallets_from_storage().expect("Failed to list wallets");
//     assert_eq!(wallets.len(), 1);
//     assert_eq!(wallets[0].wallet_name, "test_wallet_1");

//     // Retrieve and decrypt the private key
//     let decrypted_pk = get_decrypted_private_key_from_storage("test_wallet_1", "master_pass")
//         .expect("Failed to decrypt private key");
//     assert_eq!(decrypted_pk, pk);
// }

// #[tokio::test]
// async fn test_remove_wallet() {
//     let _dir = setup_test_storage("secure_pass_123");
//     initialize_wallet_storage("secure_pass_123").expect("Storage initialization failed");

//     let pk = "0x1a2b3c4d5e6f7a8b9c0d1e2f3a4b5c6d7e8f9a0b1c2d3e4f5a6b7c8d9e0f1a2b";
//     let wallet_info = import_wallet(pk).expect("Failed to import wallet");

//     // Add a wallet
//     add_wallet_to_storage("wallet_to_remove".to_string(), pk.to_string(), wallet_info.address, "secure_pass_123")
//         .expect("Failed to add wallet");

//     // Confirm it's there
//     let wallets_before = list_wallets_from_storage().unwrap();
//     assert_eq!(wallets_before.len(), 1);

//     // Remove it
//     let removed = remove_wallet_from_storage("wallet_to_remove").expect("Failed to remove wallet");
//     assert!(removed);

//     // Confirm it's gone
//     let wallets_after = list_wallets_from_storage().unwrap();
//     assert!(wallets_after.is_empty());

//     // Try to remove a non-existent wallet
//     let removed_again = remove_wallet_from_storage("non_existent_wallet").unwrap();
//     assert!(!removed_again);
// }

// #[tokio::test]
// async fn test_password_protection() {
//     let _dir = setup_test_storage("correct_password");
//     initialize_wallet_storage("correct_password").expect("Storage initialization failed");

//     let pk = "0x7f0d4c977cf0b0891798702e6bd740dc2d9aa6195be2365ee947a3c6a08a38fa";
//     let wallet_info = import_wallet(pk).expect("Failed to import wallet");

//     add_wallet_to_storage("protected_wallet".to_string(), pk.to_string(), wallet_info.address, "correct_password")
//         .expect("Failed to add wallet");

//     // Attempt to access with wrong password
//     let result = get_decrypted_private_key_from_storage("protected_wallet", "wrong_password");
//     assert!(result.is_err());
//     assert!(result.unwrap_err().to_string().contains("Invalid master password"));
// }
