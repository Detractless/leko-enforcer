//! Partner keys: the shared vectors verify, a wrong secret fails, and a decomposed spelling of a
//! secret gives the same result as the composed one.

mod vector_files;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use leko_core::domain::key_derivation::{create_key, derive_hash, matches_key, KeyError};
use leko_core::models::specs::Key;
use unicode_normalization::UnicodeNormalization;
use vector_files::read_vector;

fn key_of(vector: &serde_json::Value) -> Key {
    let key = &vector["key"];
    Key {
        alg: key["alg"].as_str().expect("alg").to_string(),
        iterations: key["iterations"].as_i64(),
        salt: key["salt"].as_str().map(str::to_string),
        hash: key["hash"].as_str().expect("hash").to_string(),
    }
}

#[test]
fn key_vectors_verify_and_normalise() {
    let doc = read_vector("key-vectors.json");
    let vectors = doc["vectors"].as_array().expect("vectors");
    assert_eq!(vectors.len(), 2, "the file holds two vectors");
    for vector in vectors {
        let name = vector["name"].as_str().expect("name");
        let secret = vector["secret"].as_str().expect("secret");
        let key = key_of(vector);
        assert_eq!(matches_key(secret, &key), Ok(true), "{name}: the secret verifies");
        assert_eq!(secret.nfc().collect::<String>(), vector["secretNfc"].as_str().expect("secretNfc"), "{name}: secretNfc is the NFC form");
        // A different secret fails.
        assert_eq!(matches_key(&format!("{secret}x"), &key), Ok(false), "{name}: a wrong secret fails");
        assert_eq!(matches_key("not the secret", &key), Ok(false), "{name}: another wrong secret fails");
        // A decomposed spelling gives the same result.
        let decomposed: String = secret.nfd().collect();
        assert_eq!(matches_key(&decomposed, &key), Ok(true), "{name}: NFD spelling verifies");
        if name == "accented" {
            assert_ne!(decomposed, secret, "the accented vector really has a decomposed spelling");
        }
        // The same salt and iterations make the same key.
        let salt = STANDARD.decode(key.salt.as_deref().expect("salt")).expect("salt is base64");
        let made = create_key(secret, &salt, key.iterations.expect("iterations")).expect("a key is made");
        assert_eq!(made, key, "{name}: creating the key gives the vector's key");
    }
}

#[test]
fn unusable_input_is_an_error_not_a_panic() {
    let salt = [7u8; 16];
    assert_eq!(derive_hash("", &salt, 100_000), Err(KeyError::EmptySecret));
    assert_eq!(derive_hash("secret", &salt[..15], 100_000), Err(KeyError::BadSalt));
    assert_eq!(derive_hash("secret", &salt, 99_999), Err(KeyError::BadIterations));
    assert_eq!(derive_hash("secret", &salt, 600_001), Err(KeyError::BadIterations));
    assert_eq!(derive_hash("secret", &salt, -1), Err(KeyError::BadIterations));
    let unsalted = Key { alg: "sha256".into(), iterations: None, salt: None, hash: "00".repeat(32) };
    assert_eq!(matches_key("secret", &unsalted), Err(KeyError::NotPbkdf2));
    let bad_salt = Key { alg: "pbkdf2-sha256".into(), iterations: Some(100_000), salt: Some("!!not base64!!".into()), hash: "00".repeat(32) };
    assert_eq!(matches_key("secret", &bad_salt), Err(KeyError::BadSalt));
}
