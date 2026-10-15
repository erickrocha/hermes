use uuid::Uuid;

pub fn bytes_para_string(bytes: Vec<u8>) -> String {
    Uuid::from_slice(&bytes)
        .map(|u| u.to_string())
        .unwrap_or_default()
}

pub fn string_to_bytes(uuid_str: &str) -> Vec<u8> {
    Uuid::parse_str(uuid_str)
        .map(|u| u.as_bytes().to_vec())
        .unwrap_or_else(|_| vec![0; 16])
}

/// `TRM-433`: lower-case, accent-free, punctuation-free, single-spaced -- the
/// one normalisation every name-keyed question is asked through (a service's
/// identity within a triage, a catalogue entry's uniqueness, an inspection
/// model match), so that "Lavagem  Externa" and "lavagem externa" are one name.
pub fn normalize_name(text: &str) -> String {
    use unicode_normalization::UnicodeNormalization;
    let stripped: String = text.nfd().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
    stripped.to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod normalize_tests {
    use super::normalize_name;

    #[test]
    fn a_name_is_one_name_whatever_its_accents_case_spacing_or_punctuation() {
        assert_eq!(normalize_name("  Limpeza  Interna. "), "limpeza interna");
        assert_eq!(normalize_name("LIMPEZA interna"), normalize_name("limpeza  interna"));
        assert_eq!(normalize_name("Higienização (WC)"), "higienizacao wc");
        assert_ne!(normalize_name("Lavagem externa"), normalize_name("Lavagem interna"));
    }
}
