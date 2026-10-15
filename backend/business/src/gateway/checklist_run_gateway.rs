use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::checklist_answer::{ChecklistAnswer, ChecklistAnswerEntityMapper};
use crate::domain::checklist_run::{ChecklistRun, ChecklistRunEntityMapper};
use crate::domain::checklist_template::ChecklistTemplate;
use crate::domain::checklist_template_item::{
    ChecklistTemplateItem, ChecklistTemplateItemEntityMapper,
};
use crate::domain::user::User;
use crate::domain::vehicle::Vehicle;
use crate::domain::work_order::{WorkOrder, WorkOrderEntityMapper};
use crate::domain::work_order_item::{WorkOrderItem, WorkOrderItemEntityMapper};
use crate::gateway::checklist_answer_gateway::ChecklistAnswerGateway;
use crate::gateway::checklist_template_gateway::ChecklistTemplateGateway;
use crate::gateway::checklist_template_item_gateway::ChecklistTemplateItemGateway;
use crate::gateway::user_gateway::{UserGateway, UserLookupStore};
use crate::gateway::vehicle_gateway::{VehicleGateway, VehicleLookupStore};
use crate::gateway::work_order_gateway::WorkOrderGateway;
use crate::gateway::work_order_item_gateway::WorkOrderItemGateway;
use entity::checklist_run_entity;
use entity::prelude::ChecklistRunEntity as ChecklistRunQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder, TransactionTrait,
};

#[async_trait]
pub trait ChecklistRunStore: Send + Sync {
    async fn find_template(&self, id: i64) -> Result<Option<ChecklistTemplate>, BusinessError>;
    async fn find_vehicle(&self, id: i64) -> Result<Option<Vehicle>, BusinessError>;
    async fn find_driver(&self, id: i64) -> Result<Option<User>, BusinessError>;
    async fn find_run(&self, id: i64) -> Result<Option<ChecklistRun>, BusinessError>;
    async fn departures_by_driver(
        &self,
        driver_id: i64,
    ) -> Result<Vec<ChecklistRun>, BusinessError>;
    async fn returns_closing(
        &self,
        opening_ids: Vec<i64>,
    ) -> Result<Vec<ChecklistRun>, BusinessError>;
    async fn latest_departure_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<ChecklistRun>, BusinessError>;
    async fn find_template_item(
        &self,
        id: i64,
    ) -> Result<Option<ChecklistTemplateItem>, BusinessError>;
    async fn list_answers(&self, run_id: i64) -> Result<Vec<ChecklistAnswer>, BusinessError>;
    async fn create_run_with_answers(
        &self,
        run: ChecklistRun,
        answers: Vec<ChecklistAnswer>,
    ) -> Result<(ChecklistRun, Vec<ChecklistAnswer>), BusinessError>;
    async fn find_run_by_uuid(&self, uuid: String) -> Result<Option<ChecklistRun>, BusinessError>;
    async fn save_answer(&self, answer: ChecklistAnswer) -> Result<ChecklistAnswer, BusinessError>;
    async fn save_work_order(&self, work_order: WorkOrder) -> Result<WorkOrder, BusinessError>;
    async fn save_work_order_item(
        &self,
        item: WorkOrderItem,
    ) -> Result<WorkOrderItem, BusinessError>;
}

/// `HRMS-652` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct ChecklistRunGateway {
    db: DbConn,
}

impl ChecklistRunGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    /// `EPIC-CK-04-S01`: the use case writes the run and its answers in one
    /// transaction started from here -- the same `PD-027`-style
    /// all-or-nothing technique `ChecklistTemplateGateway::db` already uses.
    pub fn db(&self) -> &DbConn {
        &self.db
    }
}

pub struct ChecklistRunDatabaseGateway {
    runs: ChecklistRunGateway,
    templates: ChecklistTemplateGateway,
    template_items: ChecklistTemplateItemGateway,
    answers: ChecklistAnswerGateway,
    vehicles: VehicleGateway,
    drivers: UserGateway,
    work_orders: WorkOrderGateway,
    work_order_items: WorkOrderItemGateway,
}

impl ChecklistRunDatabaseGateway {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        runs: ChecklistRunGateway,
        templates: ChecklistTemplateGateway,
        template_items: ChecklistTemplateItemGateway,
        answers: ChecklistAnswerGateway,
        vehicles: VehicleGateway,
        drivers: UserGateway,
        work_orders: WorkOrderGateway,
        work_order_items: WorkOrderItemGateway,
    ) -> Self {
        Self {
            runs,
            templates,
            template_items,
            answers,
            vehicles,
            drivers,
            work_orders,
            work_order_items,
        }
    }
}

#[async_trait]
impl ChecklistRunStore for ChecklistRunDatabaseGateway {
    async fn find_template(&self, id: i64) -> Result<Option<ChecklistTemplate>, BusinessError> {
        <ChecklistTemplateGateway as Gateway<
            ChecklistTemplate,
            entity::checklist_template_entity::Model,
            entity::checklist_template_entity::ActiveModel,
        >>::find_by_id(&self.templates, id)
        .await
        .map(|model| {
            model.map(crate::domain::checklist_template::ChecklistTemplateEntityMapper::from_model)
        })
        .map_err(database_error)
    }

    async fn find_vehicle(&self, id: i64) -> Result<Option<Vehicle>, BusinessError> {
        self.vehicles.get_by_id(id).await
    }

    async fn find_driver(&self, id: i64) -> Result<Option<User>, BusinessError> {
        self.drivers.get_by_id(id).await
    }

    async fn find_run(&self, id: i64) -> Result<Option<ChecklistRun>, BusinessError> {
        <ChecklistRunGateway as Gateway<
            ChecklistRun,
            checklist_run_entity::Model,
            checklist_run_entity::ActiveModel,
        >>::find_by_id(&self.runs, id)
        .await
        .map(|model| model.map(ChecklistRunEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn departures_by_driver(
        &self,
        driver_id: i64,
    ) -> Result<Vec<ChecklistRun>, BusinessError> {
        self.runs
            .find_departures_by_driver(driver_id)
            .await
            .map(ChecklistRunEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn returns_closing(
        &self,
        opening_ids: Vec<i64>,
    ) -> Result<Vec<ChecklistRun>, BusinessError> {
        self.runs
            .find_returns_closing(opening_ids)
            .await
            .map(ChecklistRunEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn latest_departure_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<ChecklistRun>, BusinessError> {
        self.runs
            .find_latest_departure_by_vehicle(vehicle_id)
            .await
            .map(|model| model.map(ChecklistRunEntityMapper::from_model))
            .map_err(database_error)
    }

    async fn find_template_item(
        &self,
        id: i64,
    ) -> Result<Option<ChecklistTemplateItem>, BusinessError> {
        <ChecklistTemplateItemGateway as Gateway<
            ChecklistTemplateItem,
            entity::checklist_template_item_entity::Model,
            entity::checklist_template_item_entity::ActiveModel,
        >>::find_by_id(&self.template_items, id)
        .await
        .map(|model| model.map(ChecklistTemplateItemEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn list_answers(&self, run_id: i64) -> Result<Vec<ChecklistAnswer>, BusinessError> {
        self.answers
            .find_by_run(run_id)
            .await
            .map(ChecklistAnswerEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn create_run_with_answers(
        &self,
        run: ChecklistRun,
        answers: Vec<ChecklistAnswer>,
    ) -> Result<(ChecklistRun, Vec<ChecklistAnswer>), BusinessError> {
        let transaction = self.runs.db().begin().await.map_err(database_error)?;
        let active_run = match ChecklistRunEntityMapper::build_active_model(run)
            .save(&transaction)
            .await
        {
            Ok(active) => active,
            Err(error) => {
                let _ = transaction.rollback().await;
                return Err(database_error(error));
            }
        };
        let run = ChecklistRunEntityMapper::from_active_model(active_run);
        let mut saved_answers = Vec::with_capacity(answers.len());
        for mut answer in answers {
            answer.checklist_run_id = run.id.unwrap_or_default();
            match ChecklistAnswerEntityMapper::build_active_model(answer)
                .save(&transaction)
                .await
            {
                Ok(active) => {
                    saved_answers.push(ChecklistAnswerEntityMapper::from_active_model(active))
                }
                Err(error) => {
                    let _ = transaction.rollback().await;
                    return Err(database_error(error));
                }
            }
        }
        transaction.commit().await.map_err(database_error)?;
        Ok((run, saved_answers))
    }

    async fn find_run_by_uuid(&self, uuid: String) -> Result<Option<ChecklistRun>, BusinessError> {
        <ChecklistRunGateway as Gateway<
            ChecklistRun,
            checklist_run_entity::Model,
            checklist_run_entity::ActiveModel,
        >>::find_by_uuid(&self.runs, uuid)
        .await
        .map(|model| model.map(ChecklistRunEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn save_answer(&self, answer: ChecklistAnswer) -> Result<ChecklistAnswer, BusinessError> {
        <ChecklistAnswerGateway as Gateway<
            ChecklistAnswer,
            entity::checklist_answer_entity::Model,
            entity::checklist_answer_entity::ActiveModel,
        >>::persist(&self.answers, answer)
        .await
        .map(ChecklistAnswerEntityMapper::from_active_model)
        .map_err(database_error)
    }

    async fn save_work_order(&self, work_order: WorkOrder) -> Result<WorkOrder, BusinessError> {
        <WorkOrderGateway as Gateway<
            WorkOrder,
            entity::work_order_entity::Model,
            entity::work_order_entity::ActiveModel,
        >>::persist(&self.work_orders, work_order)
        .await
        .map(WorkOrderEntityMapper::from_active_model)
        .map_err(database_error)
    }

    async fn save_work_order_item(
        &self,
        item: WorkOrderItem,
    ) -> Result<WorkOrderItem, BusinessError> {
        <WorkOrderItemGateway as Gateway<
            WorkOrderItem,
            entity::work_order_item_entity::Model,
            entity::work_order_item_entity::ActiveModel,
        >>::persist(&self.work_order_items, item)
        .await
        .map(WorkOrderItemEntityMapper::from_active_model)
        .map_err(database_error)
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Database error: {error}");
    log::error!("[ChecklistRunGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl Gateway<ChecklistRun, checklist_run_entity::Model, checklist_run_entity::ActiveModel>
    for ChecklistRunGateway
{
    async fn persist(
        &self,
        entity: ChecklistRun,
    ) -> Result<checklist_run_entity::ActiveModel, DbErr> {
        ChecklistRunEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            ChecklistRunQuery::delete_many().filter(checklist_run_entity::Column::Id.eq(id)),
            checklist_run_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<checklist_run_entity::Model>, DbErr> {
        tenant_select(
            ChecklistRunQuery::find(),
            checklist_run_entity::Column::TenantId,
        )
        .filter(checklist_run_entity::Column::Id.eq(id))
        .one(&self.db)
        .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<checklist_run_entity::Model>, DbErr> {
        tenant_select(
            ChecklistRunQuery::find(),
            checklist_run_entity::Column::TenantId,
        )
        .filter(checklist_run_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
        .one(&self.db)
        .await
    }

    async fn find_all(&self) -> Result<Vec<checklist_run_entity::Model>, DbErr> {
        tenant_select(
            ChecklistRunQuery::find(),
            checklist_run_entity::Column::TenantId,
        )
        .all(&self.db)
        .await
    }
}

impl ChecklistRunGateway {
    /// `TRM-105`: every `Departure` run this driver has ever opened, most
    /// recent first. The use case decides which of these, if any, is still
    /// open (has no `Return` naming it) -- that needs the full list, not
    /// just the latest one, since only a `Return` row proves a departure is
    /// closed.
    pub async fn find_departures_by_driver(
        &self,
        driver_id: i64,
    ) -> Result<Vec<checklist_run_entity::Model>, DbErr> {
        tenant_select(
            ChecklistRunQuery::find(),
            checklist_run_entity::Column::TenantId,
        )
        .filter(checklist_run_entity::Column::DriverId.eq(driver_id))
        .filter(checklist_run_entity::Column::ChecklistType.eq("Departure"))
        .order_by_desc(checklist_run_entity::Column::CreatedAt)
        .all(&self.db)
        .await
    }

    /// The `Return` rows (if any) that close the given `Departure` ids --
    /// at most one per id (`uq_checklist_run_opening_checklist`).
    pub async fn find_returns_closing(
        &self,
        opening_checklist_ids: Vec<i64>,
    ) -> Result<Vec<checklist_run_entity::Model>, DbErr> {
        tenant_select(
            ChecklistRunQuery::find(),
            checklist_run_entity::Column::TenantId,
        )
        .filter(checklist_run_entity::Column::OpeningChecklistId.is_in(opening_checklist_ids))
        .all(&self.db)
        .await
    }

    /// `TRM-123`-lite: the vehicle's most recent `Departure` run, whether or
    /// not it has since been closed -- the use case checks the latter via
    /// `find_returns_closing`.
    pub async fn find_latest_departure_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Option<checklist_run_entity::Model>, DbErr> {
        tenant_select(
            ChecklistRunQuery::find(),
            checklist_run_entity::Column::TenantId,
        )
        .filter(checklist_run_entity::Column::VehicleId.eq(vehicle_id))
        .filter(checklist_run_entity::Column::ChecklistType.eq("Departure"))
        .order_by_desc(checklist_run_entity::Column::CreatedAt)
        .one(&self.db)
        .await
    }
}

/// `HRMS-652` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::checklist_run_entity;
    use sea_orm::{DbBackend, EntityTrait, QueryTrait};

    fn caller(tenant_id: Option<i64>, enforce_tenant: bool) -> AuditUser {
        AuditUser {
            id: 1,
            email: "owner@example.com".to_string(),
            tenant_id,
            enforce_tenant,
        }
    }

    #[tokio::test]
    async fn a_tenant_bound_caller_reads_only_its_own_runs() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                checklist_run_entity::Entity::find(),
                checklist_run_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_run_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                checklist_run_entity::Entity::find(),
                checklist_run_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_run_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                checklist_run_entity::Entity::delete_many(),
                checklist_run_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
