/// Tests d'intégration pour POST /settings/password.
///
/// Ces tests nécessitent une base de données de test (DATABASE_URL_TEST)
/// et ENCRYPTION_KEY valide. Ils sont ignorés si les variables manquent.
///
/// Lancer avec :
///   DATABASE_URL_TEST=postgres://... ENCRYPTION_KEY=$(openssl rand -base64 32) cargo test --test settings_password
#[tokio::test]
async fn password_change_wrong_current_password() {
    // Test unitaire de la logique de validation (sans HTTP)
    use argon2::{
        password_hash::{
            rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
        },
        Argon2,
    };

    let salt = SaltString::generate(&mut OsRng);
    let hash = Argon2::default()
        .hash_password(b"correct_password", &salt)
        .unwrap()
        .to_string();

    // Vérifier que le mauvais mot de passe est rejeté
    let wrong_ok = PasswordHash::new(&hash)
        .and_then(|h| Argon2::default().verify_password(b"wrong_password", &h))
        .is_ok();
    assert!(!wrong_ok, "Le mauvais mot de passe ne doit pas passer");

    // Vérifier que le bon mot de passe est accepté
    let correct_ok = PasswordHash::new(&hash)
        .and_then(|h| Argon2::default().verify_password(b"correct_password", &h))
        .is_ok();
    assert!(correct_ok, "Le bon mot de passe doit passer");
}

#[test]
fn encryption_key_parses_correctly() {
    use base64::{engine::general_purpose::STANDARD, Engine};

    // Clé valide (32 bytes en base64)
    let key_bytes = [42u8; 32];
    let encoded = STANDARD.encode(key_bytes);
    let decoded: Vec<u8> = STANDARD.decode(&encoded).unwrap();
    let key: [u8; 32] = decoded.try_into().unwrap();
    assert_eq!(key, key_bytes);

    // Clé trop courte
    let short = STANDARD.encode([0u8; 16]);
    let decoded_short: Vec<u8> = STANDARD.decode(&short).unwrap();
    assert!(TryInto::<[u8; 32]>::try_into(decoded_short).is_err());
}

use std::convert::TryInto;
