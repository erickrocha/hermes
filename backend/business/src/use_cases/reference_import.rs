//! PD-027: importação em massa de dados de referência.
//!
//! **Tudo ou nada.** As linhas são validadas por inteiro antes de qualquer
//! gravação; se uma falhar, nada entra e a resposta diz qual linha e por quê.
//! Importação parcial deixaria o operador sem saber o que ficou dentro e o que
//! ficou fora, e a correção seria reimportar por cima de um estado que ele não
//! consegue enxergar. Como a tela é editável antes de salvar (PD-027), devolver
//! os erros é acionável: ele corrige na grade e salva de novo.

use crate::domain::business_error::BusinessError;

/// Por que uma linha foi recusada. `row` é o índice base zero **dos dados**,
/// sem contar o cabeçalho do CSV, que é o que a grade da tela mostra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRejection {
    pub row: usize,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportOutcome {
    pub created: u64,
    pub updated: u64,
}

impl ImportRejection {
    pub fn new(row: usize, reason: impl Into<String>) -> Self {
        Self { row, reason: reason.into() }
    }
}

/// Recusa o lote inteiro, listando cada linha problemática de uma vez em vez de
/// parar na primeira — o operador corrige tudo numa passada.
pub fn rejected(rejections: Vec<ImportRejection>) -> BusinessError {
    let detail = rejections
        .iter()
        .map(|rejection| format!("row {}: {}", rejection.row + 1, rejection.reason))
        .collect::<Vec<_>>()
        .join("; ");
    BusinessError::new(format!("Import rejected, nothing was written. {detail}"))
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
    use super::{find_duplicate_keys, rejected, ImportRejection};

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
        let error = rejected(vec![
            ImportRejection::new(0, "country code must be two letters"),
            ImportRejection::new(3, "name is required"),
        ]);
        assert!(error.message.contains("row 1: country code must be two letters"));
        assert!(error.message.contains("row 4: name is required"));
        assert!(error.message.contains("nothing was written"));
    }
}
