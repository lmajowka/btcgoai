use serde::{Deserialize, Serialize};

// WalletData represents the structure of the wallets.json file
#[derive(Serialize, Deserialize, Debug)]
pub struct WalletData {
    pub wallets: Vec<String>,
}

// RangeData represents the structure of the ranges.json file
#[derive(Serialize, Deserialize, Debug)]
pub struct RangeData {
    pub ranges: Vec<Range>,
}

// Range represents a single range in the ranges.json file
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Range {
    pub min: String,
    pub max: String,
    pub status: i32,
}

// Hash160Data represents the structure of the hash160s.json file
#[derive(Serialize, Deserialize, Debug)]
pub struct Hash160Data {
    pub hash160s: Vec<String>,
} 