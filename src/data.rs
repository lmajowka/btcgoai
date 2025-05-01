use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::error::Error;
use std::fmt;

use crate::models::{WalletData, RangeData, Range, Hash160Data};

// Custom error type for data loading operations
#[derive(Debug)]
pub enum DataError {
    IoError(std::io::Error),
    JsonError(serde_json::Error),
    HexError(hex::FromHexError),
    #[allow(dead_code)]
    ConversionError(String),
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DataError::IoError(err) => write!(f, "IO error: {}", err),
            DataError::JsonError(err) => write!(f, "JSON parsing error: {}", err),
            DataError::HexError(err) => write!(f, "Hex decoding error: {}", err),
            DataError::ConversionError(msg) => write!(f, "Conversion error: {}", msg),
        }
    }
}

impl Error for DataError {}

impl From<std::io::Error> for DataError {
    fn from(err: std::io::Error) -> Self {
        DataError::IoError(err)
    }
}

impl From<serde_json::Error> for DataError {
    fn from(err: serde_json::Error) -> Self {
        DataError::JsonError(err)
    }
}

impl From<hex::FromHexError> for DataError {
    fn from(err: hex::FromHexError) -> Self {
        DataError::HexError(err)
    }
}

// Load wallet addresses from data/wallets.json
#[allow(dead_code)]
pub fn load_wallet_addresses() -> Result<Vec<String>, DataError> {
    let file = File::open("data/wallets.json")?;
    let reader = BufReader::new(file);
    let wallet_data: WalletData = serde_json::from_reader(reader)?;
    Ok(wallet_data.wallets)
}

// Load wallet hash160 values from data/hash160s.json
#[allow(dead_code)]
pub fn load_wallet_hash160s() -> Result<Vec<Vec<u8>>, DataError> {
    let path = Path::new("data/hash160s.json");
    
    if !path.exists() {
        // If the dedicated hash160s file doesn't exist, try converting from addresses
        return convert_addresses_to_hash160();
    }
    
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let hash160_data: Hash160Data = serde_json::from_reader(reader)?;
    
    // Convert hex strings to byte vectors
    let mut result = Vec::with_capacity(hash160_data.hash160s.len());
    for hex_str in hash160_data.hash160s {
        let hash160_bytes = hex::decode(hex_str)?;
        result.push(hash160_bytes);
    }
    
    Ok(result)
}

// Convert addresses to hash160 values (fallback function)
#[allow(dead_code)]
fn convert_addresses_to_hash160() -> Result<Vec<Vec<u8>>, DataError> {
    let _addresses = load_wallet_addresses()?;
    
    // This is a stub implementation that should be replaced with actual conversion logic
    // In a real implementation, you would use bitcoin crate to convert each address to hash160
    // For now, we'll just return an error to indicate this isn't implemented
    Err(DataError::ConversionError(
        "Conversion from addresses to hash160 not implemented, please create data/hash160s.json".to_string()
    ))
}

// Load ranges from data/ranges.json
#[allow(dead_code)]
pub fn load_ranges() -> Result<Vec<Range>, DataError> {
    let file = File::open("data/ranges.json")?;
    let reader = BufReader::new(file);
    let range_data: RangeData = serde_json::from_reader(reader)?;
    Ok(range_data.ranges)
} 