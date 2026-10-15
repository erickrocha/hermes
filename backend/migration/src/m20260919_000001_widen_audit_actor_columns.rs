use sea_orm_migration::prelude::*;

/// DEF-IA-06 (HRMS-124): `created_by` and `updated_by` hold the acting
/// account's e-mail address, and on `user` and `tenant` they were created as
/// `varchar(50)` while the `email` columns they are copied from are
/// `varchar(500)`.
///
/// The visible symptom was an account the owner could create and never
/// activate: `POST /user` stamps `created_by` with the *administrator's*
/// address and succeeds, but `/accept-invite` stamps `updated_by` with the
/// *invitee's* own address, so any invitee whose address was longer than 50
/// characters got a generic 400 forever. A valid address is up to 254
/// characters, so the cap was reachable by ordinary corporate mailboxes, not
/// only by pathological ones.
///
/// Widened to 500 to match the `email` columns rather than to 254: the
/// audit stamp can never then be narrower than the value it copies. Fixed
/// forward with an `ALTER`, not by editing the applied migrations (HRMS-026),
/// and idempotent in effect -- widening an already-wide column is a no-op.
#[derive(DeriveMigrationName)]
pub struct Migration;

const WIDE: u32 = 500;
const NARROW: u32 = 50;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        alter_actor_columns(manager, User::Table, User::CreatedBy, User::UpdatedBy, WIDE).await?;
        alter_actor_columns(manager, Tenant::Table, Tenant::CreatedBy, Tenant::UpdatedBy, WIDE).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        alter_actor_columns(manager, User::Table, User::CreatedBy, User::UpdatedBy, NARROW).await?;
        alter_actor_columns(manager, Tenant::Table, Tenant::CreatedBy, Tenant::UpdatedBy, NARROW).await
    }
}

async fn alter_actor_columns<T, C>(
    manager: &SchemaManager<'_>,
    table: T,
    created_by: C,
    updated_by: C,
    length: u32,
) -> Result<(), DbErr>
where
    T: IntoIden,
    C: IntoIden,
{
    manager
        .alter_table(
            Table::alter()
                .table(table)
                .modify_column(ColumnDef::new(created_by).string_len(length).null())
                .modify_column(ColumnDef::new(updated_by).string_len(length).null())
                .to_owned(),
        )
        .await
}

#[derive(DeriveIden)]
enum User {
    Table,
    CreatedBy,
    UpdatedBy,
}

#[derive(DeriveIden)]
enum Tenant {
    Table,
    CreatedBy,
    UpdatedBy,
}
