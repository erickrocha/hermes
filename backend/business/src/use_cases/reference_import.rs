//! PD-027: importação em massa de dados de referência.
//!
//! **Tudo ou nada.** As linhas são validadas por inteiro antes de qualquer
//! gravação; se uma falhar, nada entra e a resposta diz qual linha e por quê.
//! Importação parcial deixaria o operador sem saber o que ficou dentro e o que
//! ficou fora, e a correção seria reimportar por cima de um estado que ele não
//! consegue enxergar. Como a tela é editável antes de salvar (PD-027), devolver
//! os erros é acionável: ele corrige na grade e salva de novo.

use unicode_normalization::UnicodeNormalization;

/// Why a row was refused.
///
/// DEF-RD-04 (PD-031): a reason is a value, not a sentence. The `business`
/// crate has no locale — it says *what* was wrong and the HTTP layer says it
/// in the caller's language. Before this, every rejection was an English
/// string built here, so a Portuguese console showed English text under
/// Portuguese headings.
///
/// DEF-RD-03: the variants also cover the failures that used to reach the
/// operator as raw database errors (`1452 (23000) … foreign key constraint
/// fails`, `Data too long for column 'acronym' at row 1`). Those are ordinary
/// validation problems; they are now detected before the write, named in terms
/// the operator can act on, and tied to the row of the *file* rather than to
/// the row of some internal statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectionReason {
    NameRequired,
    NameTooLong,
    ProvinceRequired,
    ProvinceNotFound,
    AcronymRequired,
    AcronymTooLong,
    CountryCodeInvalid,
    DuplicateCityInFile,
    DuplicateAcronymInFile,
    /// Refused against what is already stored, rather than against another row
    /// of the same file. The operator's next move differs, so the wording must.
    CityAlreadyExists,
    ProvinceAlreadyExists,
}

impl RejectionReason {
    /// The Fluent message id the HTTP layer looks up. Stable: it is part of
    /// the contract between this crate and `web/locales/*/errors.ftl`.
    pub fn message_id(self) -> &'static str {
        match self {
            RejectionReason::NameRequired => "import-name-required",
            RejectionReason::NameTooLong => "import-name-too-long",
            RejectionReason::ProvinceRequired => "import-province-required",
            RejectionReason::ProvinceNotFound => "import-province-not-found",
            RejectionReason::AcronymRequired => "import-acronym-required",
            RejectionReason::AcronymTooLong => "import-acronym-too-long",
            RejectionReason::CountryCodeInvalid => "import-country-code-invalid",
            RejectionReason::DuplicateCityInFile => "import-duplicate-city-in-file",
            RejectionReason::DuplicateAcronymInFile => "import-duplicate-acronym-in-file",
            RejectionReason::CityAlreadyExists => "import-city-already-exists",
            RejectionReason::ProvinceAlreadyExists => "import-province-already-exists",
        }
    }

    /// Used in logs, and by any caller that has no locale to hand.
    pub fn english(self) -> &'static str {
        match self {
            RejectionReason::NameRequired => "name is required",
            RejectionReason::NameTooLong => "name is at most 255 characters",
            RejectionReason::ProvinceRequired => "province is required",
            RejectionReason::ProvinceNotFound => "province does not exist",
            RejectionReason::AcronymRequired => "acronym is required",
            RejectionReason::AcronymTooLong => "acronym is at most 10 characters",
            RejectionReason::CountryCodeInvalid => "country code must be two ASCII letters",
            RejectionReason::DuplicateCityInFile => "duplicate city within the file",
            RejectionReason::DuplicateAcronymInFile => "duplicate acronym within the file",
            RejectionReason::CityAlreadyExists => "a city with this name already exists in this province",
            RejectionReason::ProvinceAlreadyExists => "a province with this acronym already exists in this country",
        }
    }
}

/// Por que uma linha foi recusada. `row` é o índice base zero **dos dados**,
/// sem contar o cabeçalho do CSV, que é o que a grade da tela mostra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRejection {
    pub row: usize,
    pub reason: RejectionReason,
}

impl ImportRejection {
    pub fn new(row: usize, reason: RejectionReason) -> Self {
        Self { row, reason }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportOutcome {
    pub created: u64,
    pub updated: u64,
}

/// What can go wrong on a reference-data write.
///
/// The two cases differ in kind and the operator needs to tell them apart:
/// `Rejected` is "fix these rows and try again", `Unavailable` is "this is not
/// your fault". DEF-RD-03: `Unavailable` deliberately carries no detail — the
/// database error is logged, never returned, so a response cannot disclose the
/// schema, a constraint name or a SQL error code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReferenceDataError {
    Rejected(Vec<ImportRejection>),
    Unavailable,
}

impl ReferenceDataError {
    pub fn one(row: usize, reason: RejectionReason) -> Self {
        ReferenceDataError::Rejected(vec![ImportRejection::new(row, reason)])
    }

    /// Records the real cause where it belongs — the log — and answers with a
    /// failure the caller can safely be told about.
    pub fn unavailable(context: &str, error: impl std::fmt::Display) -> Self {
        log::error!("[{context}] Reference data operation failed: {error}");
        ReferenceDataError::Unavailable
    }

    /// The English rendering, for logs and for tests.
    pub fn english(&self) -> String {
        match self {
            ReferenceDataError::Unavailable => "Reference data is temporarily unavailable".to_string(),
            ReferenceDataError::Rejected(rejections) => {
                let detail = rejections
                    .iter()
                    .map(|rejection| format!("row {}: {}", rejection.row + 1, rejection.reason.english()))
                    .collect::<Vec<_>>()
                    .join("; ");
                format!("Import rejected, nothing was written. {detail}")
            }
        }
    }
}

/// The key by which two reference rows are the *same* row.
///
/// DEF-RD-02: case and accents do not make a different city. The database
/// already took that view — its collation matched `São Paulo` to `Sao Paulo`,
/// so the second spelling in a file quietly overwrote the first and a file of
/// two new cities reported "1 created, 1 updated". Comparing folded keys makes
/// the in-file check agree with the storage it is meant to protect.
pub fn fold_key(value: &str) -> String {
    value
        .nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .flat_map(|c| c.to_lowercase())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Detecta duplicatas *dentro do próprio arquivo*. Sem isto, duas linhas com a
/// mesma chave passariam na validação e a segunda sobrescreveria a primeira em
/// silêncio — o arquivo pareceria ter importado inteiro.
pub fn find_duplicate_keys<K: Eq + std::hash::Hash>(keys: impl Iterator<Item = K>) -> Vec<usize> {
    let mut seen = std::collections::HashSet::new();
    keys.enumerate()
        .filter_map(|(index, key)| (!seen.insert(key)).then_some(index))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{find_duplicate_keys, fold_key, ImportRejection, ReferenceDataError, RejectionReason};

    #[test]
    fn duplicate_rows_are_reported_by_their_later_position() {
        let keys = vec!["BR-SP", "BR-RJ", "BR-SP", "BR-MG", "BR-RJ"];
        assert_eq!(find_duplicate_keys(keys.into_iter()), vec![2, 4]);
    }

    #[test]
    fn a_file_with_no_repeats_has_no_duplicates() {
        let keys = vec!["BR-SP", "BR-RJ"];
        assert!(find_duplicate_keys(keys.into_iter()).is_empty());
    }

    #[test]
    fn the_error_numbers_rows_the_way_the_operator_sees_them() {
        // Índice 0 dos dados é a linha 1 na grade.
        let error = ReferenceDataError::Rejected(vec![
            ImportRejection::new(0, RejectionReason::CountryCodeInvalid),
            ImportRejection::new(3, RejectionReason::NameRequired),
        ]);
        let message = error.english();
        assert!(message.contains("row 1: country code must be two ASCII letters"));
        assert!(message.contains("row 4: name is required"));
        assert!(message.contains("nothing was written"));
    }

    #[test]
    fn case_and_accents_do_not_make_a_different_place() {
        // DEF-RD-02: each pair is one city, not two.
        assert_eq!(fold_key("qa-rd-Case"), fold_key("QA-RD-case"));
        assert_eq!(fold_key("São Paulo"), fold_key("Sao Paulo"));
        assert_eq!(fold_key("Brasília"), fold_key("BRASILIA"));
        assert_eq!(fold_key("Río Negro"), fold_key("rio negro"));
        assert_eq!(fold_key(" São   Paulo "), fold_key("Sao Paulo"));
    }

    #[test]
    fn genuinely_different_names_still_differ() {
        assert_ne!(fold_key("Santos"), fold_key("Santo André"));
        assert_ne!(fold_key("Campinas"), fold_key("Campina Grande"));
    }

    #[test]
    fn an_unavailable_write_never_discloses_the_database() {
        let error = ReferenceDataError::unavailable(
            "test",
            "Execution Error: 1452 (23000): Cannot add or update a child row: a foreign key constraint fails (`hermes`.`city`)",
        );
        let message = error.english();
        assert!(!message.contains("1452"), "{message}");
        assert!(!message.contains("hermes"), "{message}");
        assert!(!message.contains("foreign key"), "{message}");
    }

    #[test]
    fn every_reason_has_a_distinct_message_id() {
        let reasons = [
            RejectionReason::NameRequired,
            RejectionReason::NameTooLong,
            RejectionReason::ProvinceRequired,
            RejectionReason::ProvinceNotFound,
            RejectionReason::AcronymRequired,
            RejectionReason::AcronymTooLong,
            RejectionReason::CountryCodeInvalid,
            RejectionReason::DuplicateCityInFile,
            RejectionReason::DuplicateAcronymInFile,
            RejectionReason::CityAlreadyExists,
            RejectionReason::ProvinceAlreadyExists,
        ];
        let ids: std::collections::HashSet<_> = reasons.iter().map(|r| r.message_id()).collect();
        assert_eq!(ids.len(), reasons.len(), "message ids must be unique");
        assert!(reasons.iter().all(|r| !r.english().is_empty()));
    }
}
