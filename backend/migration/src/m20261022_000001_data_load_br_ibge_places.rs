use crate::reference_data::{add_places, remove_places};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// The municipalities `000002` left out: IBGE's list (5,571 entries, including
// Brasília) minus the 91 already loaded. `000002` stays untouched (HRMS-026);
// this migration only adds, and `down` removes only what it added.
static BR_IBGE_PLACES: &str = include_str!("../data/br_places_ibge.psv");

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        add_places(manager, "BR", BR_IBGE_PLACES).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        remove_places(manager, "BR", BR_IBGE_PLACES).await
    }
}
