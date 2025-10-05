use anyhow::Result;
use sha3::{Digest, Keccak256};

/// Calculate Keccak256 hash
pub fn keccak256(data: &[u8]) -> String {
    let mut hasher = Keccak256::new();
    hasher.update(data);
    let result = hasher.finalize();
    format!("0x{}", hex::encode(result))
}

/// Sign message with private key (placeholder)
pub fn sign_message(_message: &[u8], _private_key: &str) -> Result<String> {
    // This would implement actual signing logic
    Ok("0x".to_string())
} 