use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(GarageServiceModel::Table)
                    .add_column(
                        ColumnDef::new(GarageServiceModel::Applicability)
                            .text()
                            .not_null()
                            .default("{\"scope\":\"all\"}"),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(GarageServiceModel::Table)
                    .drop_column(GarageServiceModel::Applicability)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum GarageServiceModel {
    #[sea_orm(iden = "garage_service_model")]
    Table,
    Applicability,
}
