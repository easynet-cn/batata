use base64::{Engine, engine::general_purpose::STANDARD};
use std::collections::HashMap;

pub struct ConfigEncryptionService {
    key: Vec<u8>,
}

impl ConfigEncryptionService {
    pub fn new(key: &str) -> Self {
        let mut key_bytes = key.as_bytes().to_vec();
        if key_bytes.len() < 32 {
            key_bytes.resize(32, 0);
        } else if key_bytes.len() > 32 {
            key_bytes.truncate(32);
        }
        Self { key: key_bytes }
    }

    pub fn encrypt_value(&self, value: &str) -> Result<String, anyhow::Error> {
        let mut bytes = value.as_bytes().to_vec();
        for i in 0..bytes.len() {
            bytes[i] ^= self.key[i % 32];
        }
        Ok(STANDARD.encode(&bytes))
    }

    pub fn decrypt_value(&self, encrypted: &str) -> Result<String, anyhow::Error> {
        let bytes = STANDARD.decode(encrypted).map_err(|e| anyhow::anyhow!("Base64 decode error: {}", e))?;
        let mut result = bytes;
        for i in 0..result.len() {
            result[i] ^= self.key[i % 32];
        }
        Ok(String::from_utf8(result).map_err(|e| anyhow::anyhow!("UTF-8 decode error: {}", e))?)
    }

    pub fn encrypt_configurations(&self, configs: &HashMap<String, String>, encrypted_keys: &[String]) -> HashMap<String, String> {
        let mut result = HashMap::new();
        for (key, value) in configs {
            if encrypted_keys.contains(key) {
                if let Ok(encrypted) = self.encrypt_value(value) {
                    result.insert(key.clone(), encrypted);
                } else {
                    result.insert(key.clone(), value.clone());
                }
            } else {
                result.insert(key.clone(), value.clone());
            }
        }
        result
    }

    pub fn decrypt_configurations(&self, configs: &HashMap<String, String>, encrypted_keys: &[String]) -> HashMap<String, String> {
        let mut result = HashMap::new();
        for (key, value) in configs {
            if encrypted_keys.contains(key) {
                if let Ok(decrypted) = self.decrypt_value(value) {
                    result.insert(key.clone(), decrypted);
                } else {
                    result.insert(key.clone(), value.clone());
                }
            } else {
                result.insert(key.clone(), value.clone());
            }
        }
        result
    }
}
