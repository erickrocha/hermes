//! EPIC-IA-02-S04 (HRMS-109): one minimum password length, shared by every
//! path that sets one -- direct creation and update (`UserUseCase`),
//! change-password, and the invitation flow -- so the rule does not depend
//! on which door the caller came through. Before this, only the invitation
//! path (`AccountInviteUseCase`) enforced a minimum at all; `POST /user`,
//! `PUT /user/{id}` and `PUT /user/change-password` accepted any non-empty
//! string, including a single character.

use crate::domain::business_error::BusinessError;

pub const MIN_PASSWORD_LEN: usize = 8;

pub fn validate_length(password: &str) -> Result<(), BusinessError> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(BusinessError::new(format!(
            "Password must be at least {} characters long",
            MIN_PASSWORD_LEN
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::validate_length;

    #[test]
    fn accepts_a_password_at_exactly_the_minimum_length() {
        assert!(validate_length("12345678").is_ok());
    }

    #[test]
    fn rejects_anything_shorter() {
        assert!(validate_length("1234567").is_err());
    }

    #[test]
    fn counts_unicode_scalar_values_not_bytes() {
        // Eight 2-byte characters is 8 chars, not 16 -- byte-counting would
        // wrongly accept a 4-character password here.
        assert!(validate_length("éééééééé").is_ok());
        assert!(validate_length("éééééé").is_err());
    }
}
