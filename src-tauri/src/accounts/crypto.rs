use argon2::{Algorithm, Argon2, Params, PasswordHash, PasswordHasher, PasswordVerifier, Version};
use base64::Engine;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use qrcode::QrCode;
use qrcode::render::svg;
use rand::RngCore;
use rand::rngs::OsRng;

use crate::error::{KoraError, Result};

pub const ARGON_MEMORY_KIB: u32 = 64 * 1024;
pub const ARGON_TIME: u32 = 3;
pub const ARGON_PARALLELISM: u32 = 1;

const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const B64: base64::engine::general_purpose::GeneralPurpose =
    base64::engine::general_purpose::STANDARD;

pub fn argon2() -> Result<Argon2<'static>> {
    let params = Params::new(ARGON_MEMORY_KIB, ARGON_TIME, ARGON_PARALLELISM, None)
        .map_err(|error| KoraError::Internal(format!("Argon2: {error}")))?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}

pub fn hash_pin(pin: &str) -> Result<String> {
    let salt = argon2::password_hash::SaltString::generate(&mut OsRng);
    argon2()?
        .hash_password(pin.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| KoraError::Internal(format!("PIN Argon2: {error}")))
}

pub fn verify_pin(pin: &str, hash: &str) -> Result<bool> {
    let parsed = PasswordHash::new(hash)
        .map_err(|error| KoraError::InvalidInput(format!("hash de PIN inválido: {error}")))?;
    Ok(argon2()?.verify_password(pin.as_bytes(), &parsed).is_ok())
}

pub fn derive_key(pin: &str, salt: &[u8]) -> Result<[u8; 32]> {
    let mut output = [0u8; 32];
    argon2()?
        .hash_password_into(pin.as_bytes(), salt, &mut output)
        .map_err(|error| KoraError::Internal(format!("derivación de clave: {error}")))?;
    Ok(output)
}

pub fn encrypt_secret(pin: &str, account_id: &str, plaintext: &[u8]) -> Result<(String, String)> {
    let key = derive_key(pin, account_id.as_bytes())?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let ciphertext = cipher
        .encrypt(Nonce::from_slice(&nonce_bytes), plaintext)
        .map_err(|_| KoraError::Internal("no se pudo cifrar el secreto".into()))?;
    Ok((B64.encode(ciphertext), B64.encode(nonce_bytes)))
}

pub fn decrypt_secret(
    pin: &str,
    account_id: &str,
    ciphertext: &str,
    nonce: &str,
) -> Result<Vec<u8>> {
    let key = derive_key(pin, account_id.as_bytes())?;
    let cipher = ChaCha20Poly1305::new(Key::from_slice(&key));
    let ciphertext = B64
        .decode(ciphertext)
        .map_err(|error| KoraError::InvalidInput(format!("secreto cifrado inválido: {error}")))?;
    let nonce = B64
        .decode(nonce)
        .map_err(|error| KoraError::InvalidInput(format!("nonce inválido: {error}")))?;
    if nonce.len() != 12 {
        return Err(KoraError::InvalidInput("nonce inválido".into()));
    }
    cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .map_err(|_| KoraError::InvalidInput("PIN incorrecto o secreto dañado".into()))
}

pub fn new_account_key() -> SigningKey {
    SigningKey::generate(&mut OsRng)
}

pub fn account_public_key(key: &SigningKey) -> String {
    hex::encode(key.verifying_key().to_bytes())
}

pub fn sign_bytes(key: &SigningKey, data: &[u8]) -> String {
    hex::encode(key.sign(data).to_bytes())
}

pub fn verify_bytes(public_key: &str, data: &[u8], signature: &str) -> bool {
    let Ok(public_bytes) = hex::decode(public_key) else {
        return false;
    };
    let Ok(signature_bytes) = hex::decode(signature) else {
        return false;
    };
    let Ok(public_array) = <[u8; 32]>::try_from(public_bytes.as_slice()) else {
        return false;
    };
    let Ok(signature_array) = <[u8; 64]>::try_from(signature_bytes.as_slice()) else {
        return false;
    };
    let Ok(verifying) = VerifyingKey::from_bytes(&public_array) else {
        return false;
    };
    verifying
        .verify(data, &Signature::from_bytes(&signature_array))
        .is_ok()
}

pub fn random_code() -> String {
    let mut bytes = [0u8; 5];
    OsRng.fill_bytes(&mut bytes);
    crockford_encode(&bytes)
}

pub fn crockford_encode(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(8);
    let mut buffer = 0u64;
    for byte in bytes.iter().take(5) {
        buffer = (buffer << 8) | *byte as u64;
    }
    for index in (0..8).rev() {
        output.push(CROCKFORD[((buffer >> (index * 5)) & 31) as usize] as char);
    }
    output
}

pub fn crockford_decode(value: &str) -> Option<[u8; 5]> {
    if value.len() != 8 {
        return None;
    }
    let mut buffer = 0u64;
    for character in value.chars() {
        let character = character.to_ascii_uppercase();
        let digit = match character {
            'O' => 0,
            'I' | 'L' => 1,
            other => CROCKFORD.iter().position(|item| *item as char == other)? as u64,
        };
        buffer = (buffer << 5) | digit;
    }
    let mut output = [0u8; 5];
    for index in (0..5).rev() {
        output[index] = (buffer & 0xff) as u8;
        buffer >>= 8;
    }
    Some(output)
}

pub fn validate_username(username: &str) -> Result<()> {
    let valid = (2..=32).contains(&username.len())
        && username.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        });
    if valid {
        Ok(())
    } else {
        Err(KoraError::InvalidInput(
            "el usuario debe tener 2-32 caracteres: a-z, 0-9 o _".into(),
        ))
    }
}

pub fn connection_key(username: &str, code: &str) -> String {
    format!("{username}-{}-{}", &code[..4], &code[4..])
}

pub fn parse_connection_key(value: &str) -> Option<(String, String)> {
    let value = value.trim().to_ascii_uppercase();
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 3 {
        return None;
    }
    let username = parts[0].to_ascii_lowercase();
    validate_username(&username).ok()?;
    let code = format!("{}{}", parts[1], parts[2]);
    crockford_decode(&code)?;
    Some((username, code))
}

pub fn share_key(scope: &str, code: &str) -> String {
    format!(
        "SHARE-{}-{}-{}",
        scope.to_ascii_uppercase(),
        &code[..4],
        &code[4..]
    )
}

pub fn parse_share_key(value: &str) -> Option<(String, String)> {
    let value = value.trim().to_ascii_uppercase();
    let parts: Vec<&str> = value.split('-').collect();
    if parts.len() != 4 || parts[0] != "SHARE" {
        return None;
    }
    let scope = parts[1].to_ascii_lowercase();
    if !matches!(scope.as_str(), "library" | "book" | "voice" | "audiobook") {
        return None;
    }
    let code = format!("{}{}", parts[2], parts[3]);
    crockford_decode(&code)?;
    Some((scope, code))
}

pub fn qr_svg(uri: &str) -> Result<String> {
    let code =
        QrCode::new(uri.as_bytes()).map_err(|error| KoraError::Internal(format!("QR: {error}")))?;
    Ok(code
        .render::<svg::Color>()
        .min_dimensions(220, 220)
        .dark_color(svg::Color("#182026"))
        .light_color(svg::Color("#ffffff"))
        .build())
}

pub fn pairing_uri(username: &str, key: &str, account_hint: &str, expires_at: i64) -> String {
    format!("kora://pair?v=1&u={username}&k={key}&a={account_hint}&exp={expires_at}")
}

/// Código efímero de vinculación. Seis dígitos son suficientes porque el
/// ticket caduca en dos minutos y solo admite cinco intentos.
pub fn random_pairing_code() -> String {
    use rand::Rng;
    let mut rng = OsRng;
    let value = rng.gen_range(0..1_000_000u32);
    format!("{value:06}")
}

/// Deriva la contraseña SPAKE2 del ticket efímero. No usa Argon2: el ticket
/// vive en memoria, es de un solo uso y está limitado a cinco intentos, así
/// que un KDF rápido evita que un cliente no autenticado pueda agotar CPU.
pub fn pairing_password_v2(code: &str, ticket_id: &str) -> Result<[u8; 32]> {
    use hkdf::Hkdf;
    use sha2::Sha256;

    let hkdf = Hkdf::<Sha256>::new(Some(ticket_id.as_bytes()), code.as_bytes());
    let mut output = [0u8; 32];
    hkdf.expand(b"kora-pair-v2", &mut output)
        .map_err(|error| KoraError::Internal(format!("HKDF pairing: {error}")))?;
    Ok(output)
}

#[derive(Debug, Clone)]
pub struct PairingLink {
    pub ticket_id: String,
    pub code: String,
    pub hosts: Vec<String>,
    pub port: u16,
    pub fingerprint: String,
    pub expires_at: i64,
}

pub fn pairing_uri_v2(
    ticket_id: &str,
    code: &str,
    hosts: &[String],
    port: u16,
    fingerprint: &str,
    expires_at: i64,
) -> String {
    format!(
        "kora://pair?v=2&t={ticket_id}&c={code}&h={}&p={port}&f={fingerprint}&exp={expires_at}",
        hosts.join(",")
    )
}

pub fn parse_pairing_link(value: &str) -> Option<PairingLink> {
    let (kind, params) = parse_uri(value)?;
    if kind != "pair" || param(&params, "v")? != "2" {
        return None;
    }
    let ticket_id = param(&params, "t")?.to_string();
    let code = param(&params, "c")?.to_string();
    if ticket_id.is_empty() || code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let hosts = param(&params, "h")
        .map(|hosts| {
            hosts
                .split(',')
                .map(str::trim)
                .filter(|host| !host.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let port = param(&params, "p")?.parse().ok()?;
    let fingerprint = param(&params, "f")?.to_string();
    let expires_at = param(&params, "exp")?.parse().ok()?;
    if fingerprint.is_empty() {
        return None;
    }
    Some(PairingLink {
        ticket_id,
        code,
        hosts,
        port,
        fingerprint,
        expires_at,
    })
}

pub fn share_uri(key: &str, account_hint: &str, expires_at: i64) -> String {
    format!("kora://share?v=1&k={key}&a={account_hint}&exp={expires_at}")
}

pub fn parse_uri(uri: &str) -> Option<(String, Vec<(String, String)>)> {
    let (scheme, rest) = uri.trim().split_once("://")?;
    let kind = match scheme.to_ascii_lowercase().as_str() {
        "kora" => rest.split('?').next()?.to_string(),
        _ => return None,
    };
    let query = rest.split_once('?')?.1;
    let mut params = Vec::new();
    for item in query.split('&') {
        let (key, value) = item.split_once('=')?;
        params.push((key.to_string(), value.to_string()));
    }
    Some((kind, params))
}

pub fn param<'a>(params: &'a [(String, String)], key: &str) -> Option<&'a str> {
    params
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value.as_str())
}

pub fn sas_from_key(key: &[u8], transcript: &[u8]) -> Result<String> {
    use hkdf::Hkdf;
    use sha2::Sha256;
    let hkdf = Hkdf::<Sha256>::new(Some(transcript), key);
    let mut output = [0u8; 8];
    hkdf.expand(b"sas", &mut output)
        .map_err(|error| KoraError::Internal(format!("HKDF SAS: {error}")))?;
    let value = u32::from_be_bytes([output[0], output[1], output[2], output[3]]) % 1_000_000;
    Ok(format!("{value:06}"))
}

pub fn device_material(account_id: &str, device_id: &str, public_key: &str) -> Vec<u8> {
    format!("kora-device-v1|{account_id}|{device_id}|{public_key}").into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crockford_round_trip() {
        let code = random_code();
        let bytes = crockford_decode(&code).expect("code");
        assert_eq!(crockford_encode(&bytes), code);
        assert_eq!(code.len(), 8);
    }

    #[test]
    fn pin_hashes_and_encrypts_secrets() {
        let hash = hash_pin("270527").expect("hash");
        assert!(verify_pin("270527", &hash).expect("verify"));
        assert!(!verify_pin("000000", &hash).expect("verify"));
        let (cipher, nonce) = encrypt_secret("270527", "account-1", b"secreto").expect("cifrar");
        assert_eq!(
            decrypt_secret("270527", "account-1", &cipher, &nonce).expect("descifrar"),
            b"secreto"
        );
        assert!(decrypt_secret("000000", "account-1", &cipher, &nonce).is_err());
    }

    #[test]
    fn pairing_link_v2_round_trips() {
        let uri = pairing_uri_v2(
            "ticket-1",
            "482913",
            &["192.168.1.24".into(), "fe80::1%2".into()],
            47821,
            "abc123",
            123,
        );
        let link = parse_pairing_link(&uri).expect("link");
        assert_eq!(link.ticket_id, "ticket-1");
        assert_eq!(link.code, "482913");
        assert_eq!(link.hosts.len(), 2);
        assert_eq!(link.port, 47821);
        assert_eq!(link.fingerprint, "abc123");
        let password_a = pairing_password_v2("482913", "nonce").expect("password");
        let password_b = pairing_password_v2("482913", "nonce").expect("password");
        assert_eq!(password_a, password_b);
        assert_ne!(
            password_a,
            pairing_password_v2("482913", "otro-nonce").expect("password")
        );
    }

    #[test]
    fn pairing_code_has_six_digits() {
        for _ in 0..20 {
            let code = random_pairing_code();
            assert_eq!(code.len(), 6);
            assert!(code.chars().all(|character| character.is_ascii_digit()));
        }
    }

    #[test]
    fn formats_keys_and_uris() {
        let code = "ABCD2345";
        let key = connection_key("edgar", code);
        assert_eq!(key, "edgar-ABCD-2345");
        assert_eq!(
            parse_connection_key(&key).expect("parse").0,
            "edgar".to_string()
        );
        let share = share_key("book", code);
        assert_eq!(share, "SHARE-BOOK-ABCD-2345");
        assert_eq!(parse_share_key(&share).expect("share").0, "book");
        let audiobook = share_key("audiobook", code);
        assert_eq!(
            parse_share_key(&audiobook).expect("share de audiolibro").0,
            "audiobook"
        );
        let uri = pairing_uri("edgar", &key, "a1b2c3", 123);
        let (kind, params) = parse_uri(&uri).expect("uri");
        assert_eq!(kind, "pair");
        assert_eq!(param(&params, "u"), Some("edgar"));
        assert_eq!(param(&params, "exp"), Some("123"));
    }
}
