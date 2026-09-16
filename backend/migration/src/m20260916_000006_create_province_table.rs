use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Province::Table)
                    .if_not_exists()
                    .col(pk_auto(Province::Id).integer())
                    .col(binary_len_uniq(Province::Uuid, 16))
                    .col(string_len(Province::Acronym, 10).not_null())
                    .col(string_len(Province::Name, 255).not_null())
                    .col(string_len(Province::CountryCode,2).not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_province_country_acronym")
                    .table(Province::Table)
                    .col(Province::CountryCode)
                    .col(Province::Acronym)
                    .unique()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Province::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Province {
    Table,
    Id,
    Uuid,
    Acronym,
    Name,
    CountryCode
}
