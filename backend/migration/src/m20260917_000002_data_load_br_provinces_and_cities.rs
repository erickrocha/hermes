use crate::reference_data::{seed_country, unseed_country};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// The 27 Brazilian federative units (26 states + the Federal District),
// ids 1-27. Migration `000008`'s own comment already reserved this range --
// "IDs continue after the 27 Brazilian provinces loaded by migration
// 000022" -- from before this repository was forked and renamed; that
// original migration never made it into this migration set, so the range
// it named was unused until now.
static BR_PROVINCES: &[(i32, &str, &str)] = &[
    (1, "AC", "Acre"),
    (2, "AL", "Alagoas"),
    (3, "AM", "Amazonas"),
    (4, "AP", "Amapá"),
    (5, "BA", "Bahia"),
    (6, "CE", "Ceará"),
    (7, "DF", "Distrito Federal"),
    (8, "ES", "Espírito Santo"),
    (9, "GO", "Goiás"),
    (10, "MA", "Maranhão"),
    (11, "MG", "Minas Gerais"),
    (12, "MS", "Mato Grosso do Sul"),
    (13, "MT", "Mato Grosso"),
    (14, "PA", "Pará"),
    (15, "PB", "Paraíba"),
    (16, "PE", "Pernambuco"),
    (17, "PI", "Piauí"),
    (18, "PR", "Paraná"),
    (19, "RJ", "Rio de Janeiro"),
    (20, "RN", "Rio Grande do Norte"),
    (21, "RO", "Rondônia"),
    (22, "RR", "Roraima"),
    (23, "RS", "Rio Grande do Sul"),
    (24, "SC", "Santa Catarina"),
    (25, "SE", "Sergipe"),
    (26, "SP", "São Paulo"),
    (27, "TO", "Tocantins"),
];

// Every state capital plus the major economic centres of the most populous
// states -- 90 places across 27 provinces. This is a curated set, not an
// IBGE-complete municipality list the way `000008`'s ~31,847 US places are
// Census-complete: every Brazilian tenant can select their capital today,
// but a smaller city outside this list cannot yet be selected. Recorded as
// a known limitation (see the RD slice's decision register entry), not
// hidden as if it were the full ~5,570-municipality dataset.
static BR_PLACES: &str = include_str!("../data/br_places.psv");

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        seed_country(manager, "BR", BR_PROVINCES, BR_PLACES).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        unseed_country(manager, "BR").await
    }
}
