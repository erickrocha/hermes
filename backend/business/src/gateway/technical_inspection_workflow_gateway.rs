use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::inspection_model::InspectionModel;
use crate::domain::preventive_plan::PreventivePlan;
use crate::domain::preventive_plan_alert::PreventivePlanAlert;
use crate::domain::technical_inspection::{TechnicalInspection, TechnicalInspectionEntityMapper};
use crate::domain::technical_inspection_item::{
    TechnicalInspectionItem, TechnicalInspectionItemEntityMapper,
};
use crate::domain::vehicle::Vehicle;
use crate::domain::work_order::{WorkOrder, WorkOrderEntityMapper};
use crate::domain::work_order_item::{WorkOrderItem, WorkOrderItemEntityMapper};
use crate::gateway::inspection_model_gateway::{InspectionModelGateway, InspectionModelStore};
use crate::gateway::preventive_plan_alert_gateway::{
    PreventivePlanAlertGateway, PreventivePlanAlertStore,
};
use crate::gateway::preventive_plan_gateway::PreventivePlanGateway;
use crate::gateway::technical_inspection_gateway::TechnicalInspectionGateway;
use crate::gateway::technical_inspection_item_gateway::TechnicalInspectionItemGateway;
use crate::gateway::vehicle_gateway::{VehicleGateway, VehicleLookupStore};
use crate::gateway::work_order_gateway::WorkOrderGateway;
use crate::gateway::work_order_item_gateway::WorkOrderItemGateway;
use sea_orm::DbErr;
use sea_orm::prelude::async_trait::async_trait;

#[async_trait]
pub trait TechnicalInspectionStore: Send + Sync {
    async fn find_vehicle(&self, id: i64) -> Result<Option<Vehicle>, BusinessError>;
    async fn find_model(
        &self,
        name: &str,
        tenant_id: Option<i64>,
    ) -> Result<Option<InspectionModel>, BusinessError>;
    async fn save_inspection(
        &self,
        inspection: TechnicalInspection,
    ) -> Result<TechnicalInspection, BusinessError>;
    async fn active_work_orders_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Vec<WorkOrder>, BusinessError>;
    async fn list_work_order_items(
        &self,
        work_order_id: i64,
    ) -> Result<Vec<WorkOrderItem>, BusinessError>;
    async fn save_work_order_item(
        &self,
        item: WorkOrderItem,
    ) -> Result<WorkOrderItem, BusinessError>;
    async fn save_work_order(&self, order: WorkOrder) -> Result<WorkOrder, BusinessError>;
    async fn save_inspection_item(
        &self,
        item: TechnicalInspectionItem,
    ) -> Result<TechnicalInspectionItem, BusinessError>;
    async fn plans_by_vehicle(&self, vehicle_id: i64)
    -> Result<Vec<PreventivePlan>, BusinessError>;
    async fn alerts_by_plan(&self, plan_id: i64)
    -> Result<Vec<PreventivePlanAlert>, BusinessError>;
    async fn save_alert(
        &self,
        alert: PreventivePlanAlert,
    ) -> Result<PreventivePlanAlert, BusinessError>;
    async fn inspection_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<TechnicalInspection>, BusinessError>;
    async fn list_inspection_items(
        &self,
        inspection_id: i64,
    ) -> Result<Vec<TechnicalInspectionItem>, BusinessError>;
    async fn find_work_order_item(&self, id: i64) -> Result<Option<WorkOrderItem>, BusinessError>;
    async fn find_work_order(&self, id: i64) -> Result<Option<WorkOrder>, BusinessError>;
}

pub struct TechnicalInspectionDatabaseGateway {
    inspections: TechnicalInspectionGateway,
    items: TechnicalInspectionItemGateway,
    vehicles: VehicleGateway,
    work_orders: WorkOrderGateway,
    work_order_items: WorkOrderItemGateway,
    plans: PreventivePlanGateway,
    alerts: PreventivePlanAlertGateway,
    models: InspectionModelGateway,
}

impl TechnicalInspectionDatabaseGateway {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        inspections: TechnicalInspectionGateway,
        items: TechnicalInspectionItemGateway,
        vehicles: VehicleGateway,
        work_orders: WorkOrderGateway,
        work_order_items: WorkOrderItemGateway,
        plans: PreventivePlanGateway,
        alerts: PreventivePlanAlertGateway,
        models: InspectionModelGateway,
    ) -> Self {
        Self {
            inspections,
            items,
            vehicles,
            work_orders,
            work_order_items,
            plans,
            alerts,
            models,
        }
    }
}

#[async_trait]
impl TechnicalInspectionStore for TechnicalInspectionDatabaseGateway {
    async fn find_vehicle(&self, id: i64) -> Result<Option<Vehicle>, BusinessError> {
        self.vehicles.get_by_id(id).await
    }

    async fn find_model(
        &self,
        name: &str,
        tenant_id: Option<i64>,
    ) -> Result<Option<InspectionModel>, BusinessError> {
        <InspectionModelGateway as InspectionModelStore>::find_by_name(
            &self.models,
            name,
            tenant_id,
        )
        .await
    }

    async fn save_inspection(
        &self,
        inspection: TechnicalInspection,
    ) -> Result<TechnicalInspection, BusinessError> {
        <TechnicalInspectionGateway as Gateway<
            TechnicalInspection,
            entity::technical_inspection_entity::Model,
            entity::technical_inspection_entity::ActiveModel,
        >>::persist(&self.inspections, inspection)
        .await
        .map(TechnicalInspectionEntityMapper::from_active_model)
        .map_err(database_error)
    }

    async fn active_work_orders_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Vec<WorkOrder>, BusinessError> {
        self.work_orders
            .find_active_by_vehicle(vehicle_id)
            .await
            .map(WorkOrderEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn list_work_order_items(
        &self,
        work_order_id: i64,
    ) -> Result<Vec<WorkOrderItem>, BusinessError> {
        self.work_order_items
            .find_by_work_order(work_order_id)
            .await
            .map(WorkOrderItemEntityMapper::from_models)
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

    async fn save_inspection_item(
        &self,
        item: TechnicalInspectionItem,
    ) -> Result<TechnicalInspectionItem, BusinessError> {
        <TechnicalInspectionItemGateway as Gateway<
            TechnicalInspectionItem,
            entity::technical_inspection_item_entity::Model,
            entity::technical_inspection_item_entity::ActiveModel,
        >>::persist(&self.items, item)
        .await
        .map(TechnicalInspectionItemEntityMapper::from_active_model)
        .map_err(database_error)
    }

    async fn plans_by_vehicle(
        &self,
        vehicle_id: i64,
    ) -> Result<Vec<PreventivePlan>, BusinessError> {
        self.plans
            .find_by_vehicle(vehicle_id)
            .await
            .map(crate::domain::preventive_plan::PreventivePlanEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn alerts_by_plan(
        &self,
        plan_id: i64,
    ) -> Result<Vec<PreventivePlanAlert>, BusinessError> {
        self.alerts.list_by_plan(plan_id).await
    }

    async fn save_alert(
        &self,
        alert: PreventivePlanAlert,
    ) -> Result<PreventivePlanAlert, BusinessError> {
        self.alerts.save(alert).await
    }

    async fn inspection_by_uuid(
        &self,
        uuid: String,
    ) -> Result<Option<TechnicalInspection>, BusinessError> {
        <TechnicalInspectionGateway as Gateway<
            TechnicalInspection,
            entity::technical_inspection_entity::Model,
            entity::technical_inspection_entity::ActiveModel,
        >>::find_by_uuid(&self.inspections, uuid)
        .await
        .map(|model| model.map(TechnicalInspectionEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn list_inspection_items(
        &self,
        inspection_id: i64,
    ) -> Result<Vec<TechnicalInspectionItem>, BusinessError> {
        self.items
            .find_by_inspection(inspection_id)
            .await
            .map(TechnicalInspectionItemEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn find_work_order_item(&self, id: i64) -> Result<Option<WorkOrderItem>, BusinessError> {
        <WorkOrderItemGateway as Gateway<
            WorkOrderItem,
            entity::work_order_item_entity::Model,
            entity::work_order_item_entity::ActiveModel,
        >>::find_by_id(&self.work_order_items, id)
        .await
        .map(|model| model.map(WorkOrderItemEntityMapper::from_model))
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
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Technical inspection database error: {error}");
    log::error!("[TechnicalInspectionGateway] {message}");
    BusinessError::new(message)
}
