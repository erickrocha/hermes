use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Masks are presentation, not data: the console used to send the tenant's postal code and phone as
/// typed (`13000-000`, `(11) 99999-9999`). From here on the service stores digits only and masks are the
/// console's; this brings the rows already written in line. Not reversible -- the masks were never data.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        for column in ["postal_code", "phone"] {
            db.execute_unprepared(&format!(
                "UPDATE `tenant` SET `{column}` = NULLIF(REGEXP_REPLACE(`{column}`, '[^0-9]', ''), '') \
                 WHERE `{column}` IS NOT NULL"
            ))
            .await?;
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
