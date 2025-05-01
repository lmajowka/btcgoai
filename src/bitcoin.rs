use bitcoin::secp256k1::{Secp256k1, SecretKey};
use bitcoin::{PublicKey, PrivateKey};
use bitcoin_hashes::{hash160, Hash};
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum BitcoinError {
    Secp256k1Error(bitcoin::secp256k1::Error),
    GeneralError(String),
}

impl fmt::Display for BitcoinError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BitcoinError::Secp256k1Error(err) => write!(f, "Secp256k1 error: {}", err),
            BitcoinError::GeneralError(msg) => write!(f, "Error: {}", msg),
        }
    }
}

impl Error for BitcoinError {}

impl From<bitcoin::secp256k1::Error> for BitcoinError {
    fn from(err: bitcoin::secp256k1::Error) -> Self {
        BitcoinError::Secp256k1Error(err)
    }
}

// Pad private key to ensure it's 32 bytes by padding with leading zeros
pub fn pad_private_key(key: &[u8], target_length: usize) -> Vec<u8> {
    if key.len() >= target_length {
        return key.to_vec();
    }
    let mut padded = vec![0; target_length];
    padded[target_length - key.len()..].copy_from_slice(key);
    padded
}

// Convert a private key to a Bitcoin address
pub fn private_key_to_address(private_key_bytes: &[u8]) -> Result<String, BitcoinError> {
    // Create Secp256k1 context
    let secp = Secp256k1::new();
    
    // Create a secret key from the private key bytes
    let secret_key = SecretKey::from_slice(private_key_bytes)?;
    let priv_key = PrivateKey::new(secret_key, bitcoin::Network::Bitcoin);
    
    // Derive the public key
    let public_key = PublicKey::from_private_key(&secp, &priv_key);
    
    // Get the Bitcoin address in legacy format (P2PKH)
    let address = bitcoin::Address::p2pkh(&public_key, bitcoin::Network::Bitcoin);
    
    Ok(address.to_string())
}

// Convert a private key to WIF format
pub fn private_key_to_wif(private_key_bytes: &[u8]) -> Result<String, BitcoinError> {
    // Create a secret key from the private key bytes
    let secret_key = SecretKey::from_slice(private_key_bytes)?;
    let priv_key = PrivateKey::new(secret_key, bitcoin::Network::Bitcoin);
    
    // Convert to WIF format
    Ok(priv_key.to_wif())
}

// Convert a private key to a P2PKH Bitcoin address
pub fn private_key_to_p2pkh_address(private_key_bytes: &[u8]) -> Result<String, BitcoinError> {
    // Create Secp256k1 context
    let secp = Secp256k1::new();
    
    // Create a secret key from the private key bytes
    let secret_key = SecretKey::from_slice(private_key_bytes)?;
    let priv_key = PrivateKey::new(secret_key, bitcoin::Network::Bitcoin);
    
    // Derive the public key
    let public_key = PublicKey::from_private_key(&secp, &priv_key);
    
    // Get the Bitcoin address in legacy format (P2PKH)
    let address = bitcoin::Address::p2pkh(&public_key, bitcoin::Network::Bitcoin);
    
    Ok(address.to_string())
}

// Convert a private key to a Hash160 (RIPEMD160(SHA256(public_key)))
pub fn private_key_to_hash160(private_key_bytes: &[u8]) -> Result<Vec<u8>, BitcoinError> {
    // Create Secp256k1 context
    let secp = Secp256k1::new();
    
    // Create a secret key from the private key bytes
    let secret_key = SecretKey::from_slice(private_key_bytes)?;
    let priv_key = PrivateKey::new(secret_key, bitcoin::Network::Bitcoin);
    
    // Derive the public key (compressed)
    let public_key = PublicKey::from_private_key(&secp, &priv_key);
    
    // Get the serialized compressed public key
    let serialized_pubkey = public_key.inner.serialize();
    
    // Calculate Hash160 (RIPEMD160(SHA256(pubkey)))
    let hash = hash160::Hash::hash(&serialized_pubkey);
    
    // Convert hash to bytes
    Ok(hash.as_byte_array().to_vec())
} 