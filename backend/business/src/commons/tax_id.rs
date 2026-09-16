//! Brazilian taxpayer document (CPF/CNPJ) normalisation and check-digit validation.
//!
//! The payment provider rejects a malformed document, and it does so at the worst
//! moment — mid-checkout, with the customer waiting. Catching it here turns a
//! provider round-trip into a field error, and keeps a typo from being stored as
//! though it were an identity.
//!
//! Formatting is not identity: `123.456.789-09` and `12345678909` are the same
//! document, so everything is stored digits-only and compared that way. That is
//! also what makes the `uq_customer_tax_id` index meaningful.

/// Digits-only form of a valid CPF or CNPJ, or `None` if the document is invalid.
pub fn normalize(raw: &str) -> Option<String> {
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    let valid = match digits.len() {
        11 => is_valid_cpf(&digits),
        14 => is_valid_cnpj(&digits),
        _ => false,
    };
    valid.then_some(digits)
}

fn digits_of(value: &str) -> Vec<u32> {
    value.chars().filter_map(|c| c.to_digit(10)).collect()
}

/// Rejects the repeated-digit documents (`111...`) up front.
///
/// They satisfy the check-digit arithmetic — `00000000000` computes to valid —
/// but none of them is a real document, and they are exactly what a bored user
/// types to get past a required field.
fn all_same(digits: &[u32]) -> bool {
    digits.windows(2).all(|pair| pair[0] == pair[1])
}

fn check_digit(digits: &[u32], weights: &[u32]) -> u32 {
    let sum: u32 = digits
        .iter()
        .zip(weights)
        .map(|(digit, weight)| digit * weight)
        .sum();
    let remainder = sum % 11;
    if remainder < 2 { 0 } else { 11 - remainder }
}

fn is_valid_cpf(value: &str) -> bool {
    let digits = digits_of(value);
    if digits.len() != 11 || all_same(&digits) {
        return false;
    }
    // CPF weights count down from 10 and then from 11.
    let first: Vec<u32> = (2..=10).rev().collect();
    let second: Vec<u32> = (2..=11).rev().collect();
    check_digit(&digits[..9], &first) == digits[9]
        && check_digit(&digits[..10], &second) == digits[10]
}

fn is_valid_cnpj(value: &str) -> bool {
    let digits = digits_of(value);
    if digits.len() != 14 || all_same(&digits) {
        return false;
    }
    const FIRST: [u32; 12] = [5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
    const SECOND: [u32; 13] = [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2];
    check_digit(&digits[..12], &FIRST) == digits[12]
        && check_digit(&digits[..13], &SECOND) == digits[13]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_a_valid_cpf_in_any_formatting() {
        assert_eq!(normalize("529.982.247-25").as_deref(), Some("52998224725"));
        assert_eq!(normalize("52998224725").as_deref(), Some("52998224725"));
        assert_eq!(normalize(" 529 982 247 25 ").as_deref(), Some("52998224725"));
    }

    #[test]
    fn rejects_a_cpf_with_a_wrong_check_digit() {
        // One digit off the valid number above.
        assert!(normalize("52998224726").is_none());
        assert!(normalize("12345678901").is_none());
    }

    #[test]
    fn rejects_repeated_digits_even_though_the_arithmetic_accepts_them() {
        // These satisfy the check-digit maths, which is exactly why they need
        // their own guard rather than being left to it.
        assert!(normalize("00000000000").is_none());
        assert!(normalize("11111111111").is_none());
        assert!(normalize("99999999999999").is_none());
    }

    #[test]
    fn accepts_a_valid_cnpj_and_rejects_a_broken_one() {
        assert_eq!(
            normalize("11.222.333/0001-81").as_deref(),
            Some("11222333000181")
        );
        assert!(normalize("11222333000182").is_none());
    }

    #[test]
    fn rejects_anything_that_is_not_a_document_length() {
        assert!(normalize("").is_none());
        assert!(normalize("529982247").is_none());
        assert!(normalize("529982247250").is_none());
        assert!(normalize("not a document").is_none());
    }
}
