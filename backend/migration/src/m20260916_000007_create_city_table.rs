use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(City::Table)
                    .if_not_exists()
                    .col(pk_auto(City::Id).integer())
                    .col(binary_len_uniq(City::Uuid, 16))
                    .col(integer(City::ProvinceId).not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_city_province")
                            .from(City::Table, City::ProvinceId)
                            .to(Province::Table, Province::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .col(string_len(City::Name, 255).not_null())
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(City::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum City {
    Table,
    Id,
    Uuid,
    ProvinceId,
    Name,
}

#[derive(DeriveIden)]
enum Province {
    Table,
    Id,
}
