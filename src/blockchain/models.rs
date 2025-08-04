// src/blockchain/models.rs

use serde::{Deserialize, Serialize}; // For deriving serialization/deserialization traits.

// Defines the structure for a balance response from the blockchain client.
// `Debug` allows printing the struct for debugging.
// `Serialize` allows converting this struct to a JSON string.
// `Deserialize` allows creating this struct from a JSON string.
#[derive(Debug, Serialize, Deserialize)]
pub struct BalanceResponse {
    pub amount: String, // The balance amount, typically as a string to handle large numbers.
    pub denom: String,  // The denomination of the balance (e.g., "wei", "SEI").
}
