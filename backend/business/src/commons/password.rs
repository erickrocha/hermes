//! PD-029: Argon2id substitui o bcrypt. Hashes bcrypt já gravados continuam
//! válidos para login e são reescritos em Argon2id na primeira autenticação
//! bem-sucedida (`needs_rehash`), porque não existe como reconverter um hash
//! sem a senha em claro — só o login a tem.

use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::Argon2;

#[derive(Debug, PartialEq, Eq)]
pub enum HashError {
    Failed,
}

impl std::fmt::Display for HashError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("password hashing failed")
    }
}

pub fn hash(plaintext: &str) -> Result<String, HashError> {
    Argon2::default()
        .hash_password(plaintext.as_bytes())
        .map(|hash| hash.to_string())
        .map_err(|_| HashError::Failed)
}

/// Aceita Argon2id e bcrypt. Um hash ilegível é uma falha de verificação,
/// nunca um sucesso.
pub fn verify(plaintext: &str, stored: &str) -> bool {
    if is_bcrypt(stored) {
        return bcrypt::verify(plaintext, stored).unwrap_or(false);
    }
    match PasswordHash::new(stored) {
        Ok(parsed) => Argon2::default()
            .verify_password(plaintext.as_bytes(), &parsed)
            .is_ok(),
        Err(_) => false,
    }
}

/// Verdadeiro quando o hash gravado é legado e deve ser reescrito depois de
/// um login bem-sucedido.
pub fn needs_rehash(stored: &str) -> bool {
    is_bcrypt(stored)
}

/// DEF-IA-05 (HRMS-103): a verificação descartável que o login roda quando
/// não há conta a verificar.
///
/// Sem ela, um e-mail desconhecido ou uma conta desabilitada respondem sem
/// nunca tocar no Argon2 (~1 ms) enquanto uma senha errada paga o custo do
/// hash (~360 ms), e a diferença diz a um chamador anônimo quais endereços
/// têm conta ativa. O hash é calculado uma única vez por processo, com os
/// mesmos parâmetros de [`hash`], para que o custo seja o mesmo que o do
/// caminho real.
pub fn verify_dummy(plaintext: &str) {
    static DUMMY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    let stored = DUMMY.get_or_init(|| {
        hash("hermes-constant-work-placeholder").unwrap_or_else(|_| String::new())
    });
    let _ = verify(plaintext, stored);
}

fn is_bcrypt(stored: &str) -> bool {
    stored.starts_with("$2a$") || stored.starts_with("$2b$") || stored.starts_with("$2y$")
}

#[cfg(test)]
mod tests {
    use super::{hash, needs_rehash, verify, verify_dummy};

    #[test]
    fn the_dummy_verification_never_panics_and_never_succeeds_at_anything() {
        // DEF-IA-05: it exists for its cost, not its result. What matters is
        // that the login path can always call it.
        verify_dummy("");
        verify_dummy("anything at all");
    }

    #[test]
    fn argon2_round_trip() {
        let stored = hash("correct horse battery staple").expect("hashing succeeds");
        assert!(stored.starts_with("$argon2"));
        assert!(verify("correct horse battery staple", &stored));
        assert!(!verify("wrong password", &stored));
        assert!(!needs_rehash(&stored));
    }

    #[test]
    fn the_same_password_never_produces_the_same_hash() {
        let first = hash("repeated").expect("hashing succeeds");
        let second = hash("repeated").expect("hashing succeeds");
        assert_ne!(first, second, "salt must be random per hash");
    }

    #[test]
    fn legacy_bcrypt_still_verifies_and_is_flagged_for_rehash() {
        let legacy = bcrypt::hash("legacy secret", bcrypt::DEFAULT_COST).expect("bcrypt hashes");
        assert!(verify("legacy secret", &legacy));
        assert!(!verify("wrong password", &legacy));
        assert!(needs_rehash(&legacy));
    }

    #[test]
    fn an_unreadable_hash_never_verifies() {
        assert!(!verify("anything", ""));
        assert!(!verify("anything", "not-a-hash"));
        assert!(!verify("anything", "$argon2id$truncated"));
    }
}
