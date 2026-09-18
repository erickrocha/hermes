//! EPIC-XF-09-S01/S02 (HRMS-035, AD-019, PD-017): every table and column
//! name in English, checked on every build rather than in review. AD-019's
//! own text is explicit about why: this is a direct reaction against
//! `operacao-trm`'s Portuguese schema (`motorista`, `garagem`, `escala`),
//! and hermes is the platform meant to replace it (PD-017) -- the first
//! non-English table will most plausibly arrive carrying fleet-ops
//! vocabulary, not from a mistake made here.
//!
//! A general "is this English" check isn't mechanically decidable --
//! Portuguese words like `motorista` and `garagem` are plain ASCII
//! lowercase, indistinguishable from English ones by character set alone.
//! What *is* checkable, and is exactly the risk AD-019 names: a blocklist of
//! the specific Portuguese vocabulary operacao-trm actually uses, drawn from
//! that project's own project truth. This test also rejects non-ASCII
//! characters outright, which catches accented Portuguese
//! (`combustível`, `número`) that the blocklist's exact-word matching would
//! otherwise miss.

use sea_orm::{EntityName, EntityTrait, Iden, Iterable};

/// Portuguese terms from operacao-trm's actual schema and business
/// vocabulary (its CLAUDE.md, its `docs/sql/trm_*.sql` files, its table and
/// column names) that must never appear in a hermes identifier. Not
/// exhaustive -- extend it as the fleet-ops domain's own project truth
/// (`01-project_truth/operacao-trm/`) surfaces more terms when that domain
/// is actually built here.
const BLOCKED_PORTUGUESE_TERMS: &[&str] = &[
    "motorista", "veiculo", "garagem", "viagem", "escala", "abastecimento",
    "combustivel", "manutencao", "preventiva", "despesa", "estoque", "pedido",
    "compra", "fornecedor", "usuario", "empresa", "tanque", "checklist_motorista",
    "servico", "triagem", "ciclo", "posse", "chegada", "saida", "os_",
];

fn assert_english_identifier(kind: &str, entity: &str, identifier: &str) {
    assert!(
        identifier.is_ascii(),
        "{kind} `{identifier}` on `{entity}` contains non-ASCII characters (AD-019 requires English identifiers)"
    );
    let lower = identifier.to_ascii_lowercase();
    for term in BLOCKED_PORTUGUESE_TERMS {
        assert!(
            !lower.contains(term),
            "{kind} `{identifier}` on `{entity}` contains the Portuguese term `{term}` -- AD-019 forbids operacao-trm's vocabulary in hermes' schema"
        );
    }
}

fn assert_entity_is_english<E>(entity: E)
where
    E: EntityTrait + EntityName,
    E::Column: Iterable + Iden,
{
    let table = entity.table_name();
    assert_english_identifier("table", table, table);
    for column in E::Column::iter() {
        let name = column.to_string();
        assert_english_identifier("column", table, &name);
    }
}

#[test]
fn every_current_entity_is_named_in_english() {
    assert_entity_is_english(entity::user_entity::Entity);
    assert_entity_is_english(entity::tenant_entity::Entity);
    assert_entity_is_english(entity::business_plan_entity::Entity);
    assert_entity_is_english(entity::province_entity::Entity);
    assert_entity_is_english(entity::city_entity::Entity);
}

#[test]
#[should_panic(expected = "contains the Portuguese term")]
fn the_check_itself_actually_catches_a_blocked_term() {
    assert_english_identifier("column", "test_entity", "motorista_id");
}

#[test]
#[should_panic(expected = "contains non-ASCII characters")]
fn the_check_itself_actually_catches_non_ascii() {
    assert_english_identifier("column", "test_entity", "combustível");
}
