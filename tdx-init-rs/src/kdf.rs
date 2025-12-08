use crate::error::Result;
use crate::machine_id;
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use hkdf::Hkdf;
use sha2::Sha256;

const PASSPHRASE_LENGTH: usize = 32;

/// Derive a machine-bound passphrase from machine ID and user salt
/// Uses HKDF-SHA256 for key derivation
///
/// passphrase = HKDF-SHA256(ikm: machine_id, salt: user_salt, info: "tdx-init-v1")
pub async fn derive_passphrase(user_salt: &str) -> Result<String> {
    let machine_id = machine_id::get_machine_id().await?;

    // Use HKDF to derive a key from machine_id (as IKM) and user_salt
    let hk = Hkdf::<Sha256>::new(Some(user_salt.as_bytes()), machine_id.as_bytes());

    let mut okm = [0u8; PASSPHRASE_LENGTH];
    hk.expand(b"tdx-init-v1", &mut okm)
        .expect("PASSPHRASE_LENGTH is valid for HKDF output");

    // Encode as base64 for use as LUKS passphrase
    Ok(BASE64.encode(okm))
}

/// Generate a random salt for key derivation
pub fn generate_salt() -> String {
    use rand::Rng;
    let salt_bytes: [u8; 32] = rand::rng().random();
    BASE64.encode(salt_bytes)
}
