use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CryptoError {
    #[error("Échec du chiffrement")]
    Encrypt,
    #[error("Échec du déchiffrement — données corrompues ou clé incorrecte")]
    Decrypt,
    #[error("Données base64 invalides")]
    Base64,
    #[error("Données trop courtes (nonce manquant)")]
    TooShort,
}

/// Chiffre `plaintext` avec AES-256-GCM.
/// Retourne `base64(nonce || ciphertext)`.
pub fn encrypt(key: &[u8; 32], plaintext: &str) -> Result<String, CryptoError> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::Encrypt)?;
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|_| CryptoError::Encrypt)?;

    let mut blob = nonce.to_vec();
    blob.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(blob))
}

/// Déchiffre une valeur produite par `encrypt`.
pub fn decrypt(key: &[u8; 32], encoded: &str) -> Result<String, CryptoError> {
    let blob = STANDARD.decode(encoded).map_err(|_| CryptoError::Base64)?;

    // nonce = 12 bytes (AES-GCM standard)
    if blob.len() < 12 {
        return Err(CryptoError::TooShort);
    }
    let (nonce_bytes, ciphertext) = blob.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);

    let cipher = Aes256Gcm::new_from_slice(key).map_err(|_| CryptoError::Decrypt)?;
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| CryptoError::Decrypt)?;

    String::from_utf8(plaintext).map_err(|_| CryptoError::Decrypt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> [u8; 32] {
        [0u8; 32]
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = test_key();
        let plaintext = "https://hooks.slack.com/services/T00/B00/secret";
        let enc = encrypt(&key, plaintext).unwrap();
        let dec = decrypt(&key, &enc).unwrap();
        assert_eq!(dec, plaintext);
    }

    #[test]
    fn different_nonce_each_call() {
        let key = test_key();
        let a = encrypt(&key, "test").unwrap();
        let b = encrypt(&key, "test").unwrap();
        assert_ne!(a, b, "chaque appel doit utiliser un nonce différent");
    }

    #[test]
    fn wrong_key_fails() {
        let key1 = [0u8; 32];
        let key2 = [1u8; 32];
        let enc = encrypt(&key1, "secret").unwrap();
        assert!(decrypt(&key2, &enc).is_err());
    }

    #[test]
    fn invalid_base64_fails() {
        let key = test_key();
        assert!(decrypt(&key, "not-valid-base64!!!").is_err());
    }
}
