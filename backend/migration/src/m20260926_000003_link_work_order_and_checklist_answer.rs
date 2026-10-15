use sea_orm_migration::{prelude::*, schema::*};

#[derive(DeriveMigrationName)]
pub struct Migration;

/// `EPIC-CK-03-S02` (`HRMS-653`, `C-025`/`C-026`): the checklist-to-work-order
/// seam. `work_order.checklist_run_id` links a work order back to the
/// checklist that opened it (`TRM-115`); `checklist_answer.work_order_id`
/// lets a flagged, non-conforming answer name the work order it opened --
/// the accepted `EPIC-CK-03-S02` story's own wording. Both nullable: only a
/// `WorkOrderOrigin::Checklist` order, and only a flagged answer that
/// actually generated one, ever set them.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(WorkOrder::Table)
                    .add_column(integer_null(WorkOrder::ChecklistRunId))
                    .to_owned(),
            )
            .await?;
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_work_order_checklist_run")
                    .from(WorkOrder::Table, WorkOrder::ChecklistRunId)
                    .to(ChecklistRun::Table, ChecklistRun::Id)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(ChecklistAnswer::Table)
                    .add_column(integer_null(ChecklistAnswer::WorkOrderId))
                    .to_owned(),
            )
            .await?;
        manager
            .create_foreign_key(
                ForeignKey::create()
                    .name("fk_checklist_answer_work_order")
                    .from(ChecklistAnswer::Table, ChecklistAnswer::WorkOrderId)
                    .to(WorkOrder::Table, WorkOrder::Id)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ChecklistAnswer::Table)
                    .drop_foreign_key(Alias::new("fk_checklist_answer_work_order"))
                    .drop_column(ChecklistAnswer::WorkOrderId)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(WorkOrder::Table)
                    .drop_foreign_key(Alias::new("fk_work_order_checklist_run"))
                    .drop_column(WorkOrder::ChecklistRunId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum WorkOrder {
    Table,
    Id,
    ChecklistRunId,
}

#[derive(DeriveIden)]
enum ChecklistRun {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ChecklistAnswer {
    Table,
    WorkOrderId,
}
