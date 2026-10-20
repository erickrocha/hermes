use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-MT-04` (`C-026`, `HRMS-954`): the tolerance of the "did not appear"
/// occurrence and the dwell a manual non-execution needs, per tenant.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Setting::Table)
                    .add_column(integer(Setting::NonExecutionGraceMinutes).not_null().default(60))
                    .add_column(integer(Setting::NonExecutionMinDwellMinutes).not_null().default(10))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Setting::Table)
                    .drop_column(Setting::NonExecutionGraceMinutes)
                    .drop_column(Setting::NonExecutionMinDwellMinutes)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum Setting {
    #[sea_orm(iden = "tenant_rule_setting")]
    Table,
    NonExecutionGraceMinutes,
    NonExecutionMinDwellMinutes,
}
