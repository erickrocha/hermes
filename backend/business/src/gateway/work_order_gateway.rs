use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::preventive_plan::{PreventivePlan, PreventivePlanEntityMapper};
use crate::domain::preventive_plan_extension::{
    PreventivePlanExtension, PreventivePlanExtensionEntityMapper,
};
use crate::domain::vehicle::Vehicle;
use crate::domain::work_order::{WorkOrder, WorkOrderEntityMapper};
use crate::domain::work_order_item::{WorkOrderItem, WorkOrderItemEntityMapper};
use crate::gateway::preventive_plan_extension_gateway::PreventivePlanExtensionGateway;
use crate::gateway::preventive_plan_gateway::{PreventivePlanGateway, PreventivePlanLookupStore};
use crate::gateway::vehicle_gateway::{VehicleGateway, VehicleLookupStore};
use crate::gateway::work_order_item_gateway::WorkOrderItemGateway;
use entity::prelude::WorkOrderEntity as WorkOrderQuery;
use entity::work_order_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

#[async_trait]
pub trait WorkOrderStore: Send + Sync {
    async fn save(&self, work_order: WorkOrder) -> Result<WorkOrder, BusinessError>;
    async fn get_by_id(&self, id: i64) -> Result<Option<WorkOrder>, BusinessError>;
    async fn list_by_maintenance_plan(&self, plan_id: i64)
    -> Result<Vec<WorkOrder>, BusinessError>;
}

#[async_trait]
pub trait WorkOrderWorkflowStore: Send + Sync {
    async fn save_work_order(&self, work_order: WorkOrder) -> Result<WorkOrder, BusinessError>;
    async fn find_work_order(&self, id: i64) -> Result<Option<WorkOrder>, BusinessError>;
    async fn find_work_order_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<WorkOrder>, BusinessError>;
    async fn find_active_work_orders_by_plan(
        &self,
        plan_id: i64,
    ) -> Result<Vec<WorkOrder>, BusinessError>;
    async fn list_work_orders(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<WorkOrder>, u64), BusinessError>;
    async fn save_work_order_item(
        &self,
        item: WorkOrderItem,
    ) -> Result<WorkOrderItem, BusinessError>;
    async fn find_work_order_item(&self, id: i64) -> Result<Option<WorkOrderItem>, BusinessError>;
    async fn list_work_order_items(
        &self,
        work_order_id: i64,
    ) -> Result<Vec<WorkOrderItem>, BusinessError>;
    async fn find_vehicle(&self, id: i64) -> Result<Option<Vehicle>, BusinessError>;
    async fn find_preventive_plan(&self, id: i64) -> Result<Option<PreventivePlan>, BusinessError>;
    async fn save_preventive_plan(
        &self,
        plan: PreventivePlan,
    ) -> Result<PreventivePlan, BusinessError>;
    async fn find_extension_by_item(
        &self,
        item_id: i64,
    ) -> Result<Option<PreventivePlanExtension>, BusinessError>;
}

pub struct WorkOrderWorkflowDatabaseGateway {
    work_orders: WorkOrderGateway,
    items: WorkOrderItemGateway,
    vehicles: VehicleGateway,
    plans: PreventivePlanGateway,
    extensions: PreventivePlanExtensionGateway,
}

impl WorkOrderWorkflowDatabaseGateway {
    pub fn new(
        work_orders: WorkOrderGateway,
        items: WorkOrderItemGateway,
        vehicles: VehicleGateway,
        plans: PreventivePlanGateway,
        extensions: PreventivePlanExtensionGateway,
    ) -> Self {
        Self {
            work_orders,
            items,
            vehicles,
            plans,
            extensions,
        }
    }
}

#[async_trait]
impl WorkOrderWorkflowStore for WorkOrderWorkflowDatabaseGateway {
    async fn save_work_order(&self, work_order: WorkOrder) -> Result<WorkOrder, BusinessError> {
        <WorkOrderGateway as Gateway<
            WorkOrder,
            work_order_entity::Model,
            work_order_entity::ActiveModel,
        >>::persist(&self.work_orders, work_order)
        .await
        .map(WorkOrderEntityMapper::from_active_model)
        .map_err(database_error)
    }

    async fn find_work_order(&self, id: i64) -> Result<Option<WorkOrder>, BusinessError> {
        <WorkOrderGateway as Gateway<
            WorkOrder,
            work_order_entity::Model,
            work_order_entity::ActiveModel,
        >>::find_by_id(&self.work_orders, id)
        .await
        .map(|model| model.map(WorkOrderEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn find_work_order_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<WorkOrder>, BusinessError> {
        <WorkOrderGateway as Gateway<
            WorkOrder,
            work_order_entity::Model,
            work_order_entity::ActiveModel,
        >>::find_by_uuid(&self.work_orders, uuid)
        .await
        .map(|model| model.map(WorkOrderEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn find_active_work_orders_by_plan(
        &self,
        plan_id: i64,
    ) -> Result<Vec<WorkOrder>, BusinessError> {
        self.work_orders
            .find_active_by_preventive_plan(plan_id)
            .await
            .map(WorkOrderEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn list_work_orders(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<WorkOrder>, u64), BusinessError> {
        self.work_orders
            .find_page(page, page_size)
            .await
            .map(|(models, total)| (WorkOrderEntityMapper::from_models(models), total))
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
        >>::persist(&self.items, item)
        .await
        .map(WorkOrderItemEntityMapper::from_active_model)
        .map_err(database_error)
    }

    async fn find_work_order_item(&self, id: i64) -> Result<Option<WorkOrderItem>, BusinessError> {
        <WorkOrderItemGateway as Gateway<
            WorkOrderItem,
            entity::work_order_item_entity::Model,
            entity::work_order_item_entity::ActiveModel,
        >>::find_by_id(&self.items, id)
        .await
        .map(|model| model.map(WorkOrderItemEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn list_work_order_items(
        &self,
        work_order_id: i64,
    ) -> Result<Vec<WorkOrderItem>, BusinessError> {
        self.items
            .find_by_work_order(work_order_id)
            .await
            .map(WorkOrderItemEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn find_vehicle(&self, id: i64) -> Result<Option<Vehicle>, BusinessError> {
        self.vehicles.get_by_id(id).await
    }

    async fn find_preventive_plan(&self, id: i64) -> Result<Option<PreventivePlan>, BusinessError> {
        self.plans.get_by_id(id).await
    }

    async fn save_preventive_plan(
        &self,
        plan: PreventivePlan,
    ) -> Result<PreventivePlan, BusinessError> {
        <PreventivePlanGateway as Gateway<
            PreventivePlan,
            entity::preventive_plan_entity::Model,
            entity::preventive_plan_entity::ActiveModel,
        >>::persist(&self.plans, plan)
        .await
        .map(PreventivePlanEntityMapper::from_active_model)
        .map_err(database_error)
    }

    async fn find_extension_by_item(
        &self,
        item_id: i64,
    ) -> Result<Option<PreventivePlanExtension>, BusinessError> {
        self.extensions
            .find_by_item(item_id)
            .await
            .map(|model| model.map(PreventivePlanExtensionEntityMapper::from_model))
            .map_err(database_error)
    }
}

/// `HRMS-700` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct WorkOrderGateway {
    db: DbConn,
}

impl WorkOrderGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl WorkOrderStore for WorkOrderGateway {
    async fn save(&self, work_order: WorkOrder) -> Result<WorkOrder, BusinessError> {
        let active_model = <Self as Gateway<
            WorkOrder,
            work_order_entity::Model,
            work_order_entity::ActiveModel,
        >>::persist(self, work_order)
        .await
        .map_err(database_error)?;
        Ok(WorkOrderEntityMapper::from_active_model(active_model))
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<WorkOrder>, BusinessError> {
        let model = <Self as Gateway<
            WorkOrder,
            work_order_entity::Model,
            work_order_entity::ActiveModel,
        >>::find_by_id(self, id)
        .await
        .map_err(database_error)?;
        Ok(model.map(WorkOrderEntityMapper::from_model))
    }

    async fn list_by_maintenance_plan(
        &self,
        plan_id: i64,
    ) -> Result<Vec<WorkOrder>, BusinessError> {
        let models = WorkOrderGateway::find_by_maintenance_plan(self, plan_id)
            .await
            .map_err(database_error)?;
        Ok(WorkOrderEntityMapper::from_models(models))
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Work order database error: {error}");
    log::error!("[WorkOrderGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl Gateway<WorkOrder, work_order_entity::Model, work_order_entity::ActiveModel>
    for WorkOrderGateway
{
    async fn persist(&self, entity: WorkOrder) -> Result<work_order_entity::ActiveModel, DbErr> {
        WorkOrderEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            WorkOrderQuery::delete_many().filter(work_order_entity::Column::Id.eq(id)),
            work_order_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<work_order_entity::Model>, DbErr> {
        tenant_select(WorkOrderQuery::find(), work_order_entity::Column::TenantId)
            .filter(work_order_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<work_order_entity::Model>, DbErr> {
        tenant_select(WorkOrderQuery::find(), work_order_entity::Column::TenantId)
            .filter(work_order_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<work_order_entity::Model>, DbErr> {
        tenant_select(WorkOrderQuery::find(), work_order_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl WorkOrderGateway {
    /// `PD-028`: every work order of the caller's tenant, most recently
    /// opened first.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<work_order_entity::Model>, u64), DbErr> {
        let query = tenant_select(WorkOrderQuery::find(), work_order_entity::Column::TenantId)
            .order_by_desc(work_order_entity::Column::OpenedAt);
        fetch_page(query, &self.db, page, page_size).await
    }

    /// `TRM-306`: a preventive plan's work orders that are still open,
    /// partially resolved or awaiting parts -- i.e. not concluded/cancelled.
    pub async fn find_active_by_preventive_plan(
        &self,
        preventive_plan_id: i64,
    ) -> Result<Vec<work_order_entity::Model>, DbErr> {
        tenant_select(WorkOrderQuery::find(), work_order_entity::Column::TenantId)
            .filter(work_order_entity::Column::PreventivePlanId.eq(preventive_plan_id))
            .filter(work_order_entity::Column::Status.is_not_in(["Concluded", "Cancelled"]))
            .all(&self.db)
            .await
    }

    /// `TRM-333`: a vehicle's work orders that are not concluded or cancelled.
    pub async fn find_active_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Vec<work_order_entity::Model>, DbErr> {
        tenant_select(WorkOrderQuery::find(), work_order_entity::Column::TenantId)
            .filter(work_order_entity::Column::VehicleId.eq(vehicle_id))
            .filter(work_order_entity::Column::Status.is_not_in(["Concluded", "Cancelled"]))
            .order_by_asc(work_order_entity::Column::Id)
            .all(&self.db)
            .await
    }

    /// `EPIC-MT-03-S01`: the work orders a maintenance plan covers.
    pub async fn find_by_maintenance_plan(
        &self,
        maintenance_plan_id: i64,
    ) -> Result<Vec<work_order_entity::Model>, DbErr> {
        tenant_select(WorkOrderQuery::find(), work_order_entity::Column::TenantId)
            .filter(work_order_entity::Column::MaintenancePlanId.eq(maintenance_plan_id))
            .all(&self.db)
            .await
    }
}

/// `HRMS-700` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::work_order_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_work_orders() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                work_order_entity::Entity::find(),
                work_order_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_work_order_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                work_order_entity::Entity::find(),
                work_order_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_work_order_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                work_order_entity::Entity::delete_many(),
                work_order_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
