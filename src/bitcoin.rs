use bitcoin::secp256k1::{Secp256k1, SecretKey};
use bitcoin::{PrivateKey, Address, PublicKey};
use bitcoin_hashes::{hash160, Hash};
use num_bigint::BigUint;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum BitcoinError {
    BitcoinLibError(bitcoin::Error),
    Secp256k1Error(bitcoin::secp256k1::Error),
    Other(String),
}

impl fmt::Display for BitcoinError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BitcoinError::BitcoinLibError(err) => write!(f, "Bitcoin library error: {}", err),
            BitcoinError::Secp256k1Error(err) => write!(f, "Secp256k1 error: {}", err),
            BitcoinError::Other(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl Error for BitcoinError {}

impl From<bitcoin::Error> for BitcoinError {
    fn from(err: bitcoin::Error) -> Self {
        BitcoinError::BitcoinLibError(err)
    }
}

impl From<bitcoin::secp256k1::Error> for BitcoinError {
    fn from(err: bitcoin::secp256k1::Error) -> Self {
        BitcoinError::Secp256k1Error(err)
    }
}

// Converts a raw private key to WIF format
pub fn private_key_to_wif(private_key_bytes: &[u8]) -> Result<String, BitcoinError> {
    // Create Secp256k1 context
    let _secp = Secp256k1::new();
    
    // Create a secret key from the private key bytes
    let secret_key = SecretKey::from_slice(private_key_bytes)?;
    let priv_key = PrivateKey::new(secret_key, bitcoin::Network::Bitcoin);
    
    Ok(priv_key.to_wif())
}

// Convert a private key to a P2PKH address
pub fn private_key_to_p2pkh_address(private_key_bytes: &[u8]) -> Result<String, BitcoinError> {
    // Create Secp256k1 context
    let secp = Secp256k1::new();
    
    // Create a secret key from the private key bytes
    let secret_key = SecretKey::from_slice(private_key_bytes)?;
    let priv_key = PrivateKey::new(secret_key, bitcoin::Network::Bitcoin);
    
    // Generate public key from private key
    // Handle different API versions
    #[allow(deprecated)]
    let public_key = PublicKey::from_private_key(&secp, &priv_key);
    
    // Create P2PKH address from public key
    let address = Address::p2pkh(&public_key, bitcoin::Network::Bitcoin);
    
    Ok(address.to_string())
}

// Convert a private key to a Hash160 (RIPEMD160(SHA256(public_key)))
pub fn private_key_to_hash160(private_key_bytes: &[u8]) -> Result<Vec<u8>, BitcoinError> {
    // Create Secp256k1 context
    let secp = Secp256k1::new();
    
    // Create a secret key from the private key bytes
    let secret_key = SecretKey::from_slice(private_key_bytes)?;
    let priv_key = PrivateKey::new(secret_key, bitcoin::Network::Bitcoin);
    
    // Generate public key from private key
    // Handle different API versions
    #[allow(deprecated)]
    let public_key = PublicKey::from_private_key(&secp, &priv_key);
    
    // Serialize the public key
    let serialized_pubkey = public_key.inner.serialize();
    
    // Calculate Hash160 of the address
    let hash = hash160::Hash::hash(&serialized_pubkey);
    
    // Convert hash to bytes - handle different API versions
    #[allow(deprecated)]
    let hash_bytes = match hash.as_inner().to_vec() {
        bytes if !bytes.is_empty() => bytes,
        _ => {
            // Fallback for different API version
            match hash.as_ref().to_vec() {
                bytes if !bytes.is_empty() => bytes,
                _ => {
                    // Last resort
                    let hash_ref: &[u8] = hash.as_ref();
                    hash_ref.to_vec()
                }
            }
        }
    };
    
    Ok(hash_bytes)
}

// Helper function to pad a private key to 32 bytes
pub fn pad_private_key(key: &BigUint) -> Vec<u8> {
    let mut bytes = key.to_bytes_le();
    
    // Ensure key is 32 bytes (pad with zeros if necessary)
    if bytes.len() < 32 {
        let padding_needed = 32 - bytes.len();
        bytes.extend(vec![0; padding_needed]);
    } else if bytes.len() > 32 {
        // Truncate to 32 bytes if larger
        bytes.truncate(32);
    }
    
    bytes
}

/// Validate if a private key (in bytes) matches a specific Hash160 target
pub fn validate_private_key_for_hash160(key_bytes: &[u8], target_hash160: &[u8]) -> bool {
    // Ensure the key is properly formatted as a 32-byte array
    let padded_key = if key_bytes.len() < 32 {
        let mut padded = vec![0u8; 32];
        let start_idx = 32 - key_bytes.len();
        padded[start_idx..].copy_from_slice(key_bytes);
        padded
    } else if key_bytes.len() > 32 {
        key_bytes[key_bytes.len() - 32..].to_vec()
    } else {
        key_bytes.to_vec()
    };
    
    // Generate Hash160 from this key
    match private_key_to_hash160(&padded_key) {
        Ok(hash) => {
            // Compare with target hash
            if hash.len() != target_hash160.len() {
                return false;
            }
            
            // Check if all bytes match
            for i in 0..hash.len() {
                if hash[i] != target_hash160[i] {
                    return false;
                }
            }
            
            true
        },
        Err(_) => false,
    }
} 