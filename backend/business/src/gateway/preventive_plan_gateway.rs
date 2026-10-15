use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::business_error::BusinessError;
use crate::domain::preventive_plan::{PreventivePlan, PreventivePlanEntityMapper};
use crate::domain::preventive_plan_extension::{
    PreventivePlanExtension, PreventivePlanExtensionEntityMapper,
};
use crate::domain::tenant_rule_setting::RuleSettings;
use crate::domain::vehicle::{Vehicle, VehicleEntityMapper};
use crate::domain::work_order::{WorkOrder, WorkOrderEntityMapper};
use crate::domain::work_order_item::{WorkOrderItem, WorkOrderItemEntityMapper};
use crate::gateway::preventive_plan_extension_gateway::PreventivePlanExtensionGateway;
use crate::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use crate::gateway::work_order_gateway::WorkOrderGateway;
use crate::gateway::work_order_item_gateway::WorkOrderItemGateway;
use entity::prelude::PreventivePlanEntity as PreventivePlanQuery;
use entity::preventive_plan_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DbConn, DbErr, DeleteResult, EntityTrait,
    QueryFilter, QueryOrder,
};

#[async_trait]
pub trait PreventivePlanLookupStore: Send + Sync {
    async fn get_by_id(&self, id: i64) -> Result<Option<PreventivePlan>, BusinessError>;
    async fn get_by_uuid(&self, uuid: String) -> Result<Option<PreventivePlan>, BusinessError>;
}

/// `HRMS-706` (`D-09`): every read and delete goes through
/// `tenant_select`/`tenant_delete`, same as every other gateway here.
pub struct PreventivePlanGateway {
    db: DbConn,
}

impl PreventivePlanGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl PreventivePlanLookupStore for PreventivePlanGateway {
    async fn get_by_id(&self, id: i64) -> Result<Option<PreventivePlan>, BusinessError> {
        let model = <Self as Gateway<
            PreventivePlan,
            preventive_plan_entity::Model,
            preventive_plan_entity::ActiveModel,
        >>::find_by_id(self, id)
        .await
        .map_err(database_error)?;
        Ok(model.map(PreventivePlanEntityMapper::from_model))
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<PreventivePlan>, BusinessError> {
        let model = <Self as Gateway<
            PreventivePlan,
            preventive_plan_entity::Model,
            preventive_plan_entity::ActiveModel,
        >>::find_by_uuid(self, uuid)
        .await
        .map_err(database_error)?;
        Ok(model.map(PreventivePlanEntityMapper::from_model))
    }
}

#[async_trait]
pub trait PreventivePlanStore: Send + Sync {
    async fn save_plan(&self, plan: PreventivePlan) -> Result<PreventivePlan, BusinessError>;
    async fn find_plan(&self, uuid: String) -> Result<Option<PreventivePlan>, BusinessError>;
    async fn find_plan_by_vehicle_and_name(
        &self,
        vehicle_id: i64,
        name: &str,
    ) -> Result<Option<PreventivePlan>, BusinessError>;
    async fn list_plans(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<PreventivePlan>, u64), BusinessError>;
    async fn find_vehicle(&self, id: i64) -> Result<Option<Vehicle>, BusinessError>;
    async fn find_work_order(&self, id: i64) -> Result<Option<WorkOrder>, BusinessError>;
    async fn active_work_orders(&self, plan_id: i64) -> Result<Vec<WorkOrder>, BusinessError>;
    async fn save_work_order(&self, order: WorkOrder) -> Result<WorkOrder, BusinessError>;
    async fn find_work_order_item(
        &self,
        uuid: String,
    ) -> Result<Option<WorkOrderItem>, BusinessError>;
    async fn save_work_order_item(
        &self,
        item: WorkOrderItem,
    ) -> Result<WorkOrderItem, BusinessError>;
    async fn save_extension(
        &self,
        extension: PreventivePlanExtension,
    ) -> Result<PreventivePlanExtension, BusinessError>;
    async fn rules(&self, tenant_id: Option<i64>) -> Result<RuleSettings, BusinessError>;
}

pub struct PreventivePlanDatabaseGateway {
    plans: PreventivePlanGateway,
    vehicles: VehicleGateway,
    work_orders: WorkOrderGateway,
    items: WorkOrderItemGateway,
    extensions: PreventivePlanExtensionGateway,
    rules: TenantRuleSettingGateway,
}

impl PreventivePlanDatabaseGateway {
    pub fn new(
        plans: PreventivePlanGateway,
        vehicles: VehicleGateway,
        work_orders: WorkOrderGateway,
        items: WorkOrderItemGateway,
        extensions: PreventivePlanExtensionGateway,
        rules: TenantRuleSettingGateway,
    ) -> Self {
        Self {
            plans,
            vehicles,
            work_orders,
            items,
            extensions,
            rules,
        }
    }
}

#[async_trait]
impl PreventivePlanStore for PreventivePlanDatabaseGateway {
    async fn save_plan(&self, plan: PreventivePlan) -> Result<PreventivePlan, BusinessError> {
        <PreventivePlanGateway as Gateway<
            PreventivePlan,
            preventive_plan_entity::Model,
            preventive_plan_entity::ActiveModel,
        >>::persist(&self.plans, plan)
        .await
        .map(PreventivePlanEntityMapper::from_active_model)
        .map_err(database_error)
    }

    async fn find_plan(&self, uuid: String) -> Result<Option<PreventivePlan>, BusinessError> {
        <PreventivePlanGateway as Gateway<
            PreventivePlan,
            preventive_plan_entity::Model,
            preventive_plan_entity::ActiveModel,
        >>::find_by_uuid(&self.plans, uuid)
        .await
        .map(|model| model.map(PreventivePlanEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn find_plan_by_vehicle_and_name(
        &self,
        vehicle_id: i64,
        name: &str,
    ) -> Result<Option<PreventivePlan>, BusinessError> {
        self.plans
            .find_by_vehicle_and_name(vehicle_id, name)
            .await
            .map(|model| model.map(PreventivePlanEntityMapper::from_model))
            .map_err(database_error)
    }

    async fn list_plans(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<PreventivePlan>, u64), BusinessError> {
        self.plans
            .find_page(page, page_size)
            .await
            .map(|(models, total)| (PreventivePlanEntityMapper::from_models(models), total))
            .map_err(database_error)
    }

    async fn find_vehicle(&self, id: i64) -> Result<Option<Vehicle>, BusinessError> {
        <VehicleGateway as Gateway<
            Vehicle,
            entity::vehicle_entity::Model,
            entity::vehicle_entity::ActiveModel,
        >>::find_by_id(&self.vehicles, id)
        .await
        .map(|model| model.map(VehicleEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn find_work_order(&self, id: i64) -> Result<Option<WorkOrder>, BusinessError> {
        <WorkOrderGateway as Gateway<
            WorkOrder,
            entity::work_order_entity::Model,
            entity::work_order_entity::ActiveModel,
        >>::find_by_id(&self.work_orders, id)
        .await
        .map(|model| model.map(WorkOrderEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn active_work_orders(&self, plan_id: i64) -> Result<Vec<WorkOrder>, BusinessError> {
        self.work_orders
            .find_active_by_preventive_plan(plan_id)
            .await
            .map(WorkOrderEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn save_work_order(&self, order: WorkOrder) -> Result<WorkOrder, BusinessError> {
        <WorkOrderGateway as Gateway<
            WorkOrder,
            entity::work_order_entity::Model,
            entity::work_order_entity::ActiveModel,
        >>::persist(&self.work_orders, order)
        .await
        .map(WorkOrderEntityMapper::from_active_model)
        .map_err(database_error)
    }

    async fn find_work_order_item(
        &self,
        uuid: String,
    ) -> Result<Option<WorkOrderItem>, BusinessError> {
        <WorkOrderItemGateway as Gateway<
            WorkOrderItem,
            entity::work_order_item_entity::Model,
            entity::work_order_item_entity::ActiveModel,
        >>::find_by_uuid(&self.items, uuid)
        .await
        .map(|model| model.map(WorkOrderItemEntityMapper::from_model))
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

    async fn save_extension(
        &self,
        extension: PreventivePlanExtension,
    ) -> Result<PreventivePlanExtension, BusinessError> {
        <PreventivePlanExtensionGateway as Gateway<
            PreventivePlanExtension,
            entity::preventive_plan_extension_entity::Model,
            entity::preventive_plan_extension_entity::ActiveModel,
        >>::persist(&self.extensions, extension)
        .await
        .map(PreventivePlanExtensionEntityMapper::from_active_model)
        .map_err(database_error)
    }

    async fn rules(&self, tenant_id: Option<i64>) -> Result<RuleSettings, BusinessError> {
        self.rules
            .settings_for(tenant_id)
            .await
            .map_err(database_error)
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Preventive plan database error: {error}");
    log::error!("[PreventivePlanGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl Gateway<PreventivePlan, preventive_plan_entity::Model, preventive_plan_entity::ActiveModel>
    for PreventivePlanGateway
{
    async fn persist(
        &self,
        entity: PreventivePlan,
    ) -> Result<preventive_plan_entity::ActiveModel, DbErr> {
        PreventivePlanEntityMapper::build_active_model(entity)
            .save(&self.db)
            .await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            PreventivePlanQuery::delete_many().filter(preventive_plan_entity::Column::Id.eq(id)),
            preventive_plan_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<preventive_plan_entity::Model>, DbErr> {
        tenant_select(
            PreventivePlanQuery::find(),
            preventive_plan_entity::Column::TenantId,
        )
        .filter(preventive_plan_entity::Column::Id.eq(id))
        .one(&self.db)
        .await
    }

    async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<preventive_plan_entity::Model>, DbErr> {
        tenant_select(
            PreventivePlanQuery::find(),
            preventive_plan_entity::Column::TenantId,
        )
        .filter(preventive_plan_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
        .one(&self.db)
        .await
    }

    async fn find_all(&self) -> Result<Vec<preventive_plan_entity::Model>, DbErr> {
        tenant_select(
            PreventivePlanQuery::find(),
            preventive_plan_entity::Column::TenantId,
        )
        .all(&self.db)
        .await
    }
}

impl PreventivePlanGateway {
    /// `PD-028`: every preventive plan of the caller's tenant, by vehicle.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<preventive_plan_entity::Model>, u64), DbErr> {
        let query = tenant_select(
            PreventivePlanQuery::find(),
            preventive_plan_entity::Column::TenantId,
        )
        .order_by_asc(preventive_plan_entity::Column::VehicleId);
        fetch_page(query, &self.db, page, page_size).await
    }

    /// A vehicle's preventive plans.
    pub async fn find_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Vec<preventive_plan_entity::Model>, DbErr> {
        tenant_select(
            PreventivePlanQuery::find(),
            preventive_plan_entity::Column::TenantId,
        )
        .filter(preventive_plan_entity::Column::VehicleId.eq(vehicle_id))
        .all(&self.db)
        .await
    }

    /// `TRM-300`: the duplicate-refusal pre-check -- "at most one plan per
    /// vehicle and plan type."
    pub async fn find_by_vehicle_and_name(
        &self,
        vehicle_id: i64,
        plan_name: &str,
    ) -> Result<Option<preventive_plan_entity::Model>, DbErr> {
        tenant_select(
            PreventivePlanQuery::find(),
            preventive_plan_entity::Column::TenantId,
        )
        .filter(
            Condition::all()
                .add(preventive_plan_entity::Column::VehicleId.eq(vehicle_id))
                .add(preventive_plan_entity::Column::PlanName.eq(plan_name)),
        )
        .one(&self.db)
        .await
    }
}

/// `HRMS-706` (`D-09`): same technique every other gateway's own tests use.
#[cfg(test)]
mod tests {
    use crate::commons::gateway::{tenant_delete, tenant_select};
    use entity::audit::{AuditUser, run_with_user};
    use entity::preventive_plan_entity;
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
    async fn a_tenant_bound_caller_reads_only_its_own_preventive_plans() {
        let sql = run_with_user(Some(caller(Some(42), true)), async {
            tenant_select(
                preventive_plan_entity::Entity::find(),
                preventive_plan_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains("42"), "{sql}");
    }

    #[tokio::test]
    async fn a_caller_with_no_tenant_scope_reads_no_preventive_plan_at_all() {
        let sql = run_with_user(Some(caller(None, true)), async {
            tenant_select(
                preventive_plan_entity::Entity::find(),
                preventive_plan_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.contains("1 = 0"), "{sql}");
    }

    #[tokio::test]
    async fn deleting_a_preventive_plan_is_scoped_the_same_way_as_reading_one() {
        let sql = run_with_user(Some(caller(Some(7), true)), async {
            tenant_delete(
                preventive_plan_entity::Entity::delete_many(),
                preventive_plan_entity::Column::TenantId,
            )
            .build(DbBackend::MySql)
            .to_string()
        })
        .await;
        assert!(sql.to_lowercase().contains("tenant_id"), "{sql}");
        assert!(sql.contains('7'), "{sql}");
    }
}
