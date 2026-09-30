use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::{WorkOrderItemStatus, WorkOrderOrigin, WorkOrderStatus};
use crate::domain::preventive_plan::PreventivePlanEntityMapper;
use crate::domain::preventive_plan_alert::PreventivePlanAlertEntityMapper;
use crate::domain::technical_inspection::{TechnicalInspection, TechnicalInspectionEntityMapper};
use crate::domain::technical_inspection_item::{TechnicalInspectionItem, TechnicalInspectionItemEntityMapper};
use crate::domain::work_order::{WorkOrder, WorkOrderEntityMapper};
use crate::domain::work_order_item::{WorkOrderItem, WorkOrderItemEntityMapper};
use crate::gateway::inspection_model_gateway::InspectionModelGateway;
use crate::gateway::preventive_plan_alert_gateway::PreventivePlanAlertGateway;
use crate::gateway::preventive_plan_gateway::PreventivePlanGateway;
use crate::gateway::technical_inspection_gateway::TechnicalInspectionGateway;
use crate::gateway::technical_inspection_item_gateway::TechnicalInspectionItemGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use crate::gateway::work_order_gateway::WorkOrderGateway;
use crate::gateway::work_order_item_gateway::WorkOrderItemGateway;
use crate::use_cases::preventive_plan_alert_use_case::{cycle_key, pending_alerts};
use sea_orm::DbErr;
use std::collections::HashSet;

pub const NOT_A_TENANT_VEHICLE: &str = "The vehicle named does not belong to this tenant";
pub const MODEL_REQUIRED: &str = "An inspection needs an inspection model";
pub const AT_LEAST_ONE_ITEM_REQUIRED: &str = "An inspection needs at least one item";
pub const ITEM_DESCRIPTION_REQUIRED: &str = "Every inspection item needs a description";
pub const INSPECTION_NOT_FOUND: &str = "Technical inspection not found";

/// `EPIC-MT-07-S09` (`HRMS-714`, `TRM-333`): technical inspections.
pub struct TechnicalInspectionUseCase {
    inspections: TechnicalInspectionGateway,
    items: TechnicalInspectionItemGateway,
    vehicles: VehicleGateway,
    work_orders: WorkOrderGateway,
    work_order_items: WorkOrderItemGateway,
    plans: PreventivePlanGateway,
    alerts: PreventivePlanAlertGateway,
    models: InspectionModelGateway,
}

impl TechnicalInspectionUseCase {
    #[allow(clippy::too_many_arguments)] // one gateway per collaborator, wired once in the endpoint
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
        Self { inspections, items, vehicles, work_orders, work_order_items, plans, alerts, models }
    }

    /// Records an inspection. Each non-conforming item whose normalised
    /// description matches a pending item of an open work order of the same
    /// vehicle is linked to that item -- noted, never a new order, never a
    /// preventive renewal (`TRM-333`); one work order is opened for the rest.
    /// Every intermediate alert of the vehicle's plans naming this inspection
    /// model is discharged for the plan's current cycle (`TRM-325`).
    /// Returns the saved inspection, its items and whether a work order was
    /// opened.
    pub async fn submit(
        &self,
        inspection: TechnicalInspection,
        answers: Vec<TechnicalInspectionItem>,
    ) -> Result<(TechnicalInspection, Vec<TechnicalInspectionItem>, bool), BusinessError> {
        let model = inspection.inspection_model.trim().to_string();
        if model.is_empty() {
            return Err(BusinessError::new(MODEL_REQUIRED.to_string()));
        }
        if answers.is_empty() {
            return Err(BusinessError::new(AT_LEAST_ONE_ITEM_REQUIRED.to_string()));
        }
        if answers.iter().any(|a| a.description.trim().is_empty()) {
            return Err(BusinessError::new(ITEM_DESCRIPTION_REQUIRED.to_string()));
        }
        let vehicle = self.vehicles.find_by_id(inspection.vehicle_id).await.map_err(database_error)?;
        let Some(vehicle) = vehicle.filter(|v| v.tenant_id == inspection.tenant_id) else {
            return Err(BusinessError::new(NOT_A_TENANT_VEHICLE.to_string()));
        };
        // A model that says so opens and links nothing (`gera_os_nao_conforme`);
        // an unknown name behaves as a model that generates work orders.
        let generates = self
            .models
            .find_by_name(&model, inspection.tenant_id)
            .await
            .map_err(database_error)?
            .is_none_or(|m| m.generates_work_order);
        let odometer = if inspection.odometer_km > 0.0 { inspection.odometer_km } else { vehicle.odometer_km.unwrap_or(0.0) };

        let saved = self
            .inspections
            .persist(TechnicalInspection { inspection_model: model.clone(), odometer_km: odometer, ..inspection })
            .await
            .map_err(database_error)?;
        let saved = TechnicalInspectionEntityMapper::from_active_model(saved);
        let inspection_id = saved.id.unwrap_or_default();

        // `TRM-333`: pending items of the vehicle's open work orders, each usable once.
        let mut open_items = Vec::new();
        let open_orders = if generates {
            self.work_orders.find_active_by_vehicle(saved.vehicle_id).await.map_err(database_error)?
        } else {
            vec![]
        };
        for order in open_orders {
            for item in self.work_order_items.find_by_work_order(order.id).await.map_err(database_error)? {
                if matches!(item.status.as_str(), "Pending" | "AwaitingParts") {
                    open_items.push(WorkOrderItemEntityMapper::from_model(item));
                }
            }
        }
        let mut used = HashSet::new();
        let mut matched = Vec::with_capacity(answers.len());
        let mut unmatched = Vec::new();
        for (index, answer) in answers.iter().enumerate() {
            let mut target = None;
            if !answer.conforming && generates {
                let key = defect_key(&answer.description);
                target = open_items
                    .iter()
                    .find(|i| !key.is_empty() && !used.contains(&i.id) && defect_key(&i.description) == key)
                    .cloned();
                match &target {
                    Some(item) => {
                        used.insert(item.id);
                    }
                    None => unmatched.push(index),
                }
            }
            matched.push(target);
        }

        // Notes on the matched items.
        for (answer, target) in answers.iter().zip(&matched) {
            let Some(item) = target else { continue };
            let note = [Some(format!("Inspection {}", saved.inspected_at)), answer.observation.clone()]
                .into_iter()
                .flatten()
                .filter(|s| !s.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" · ");
            let observation = match &item.observation {
                Some(current) if current.contains(&note) => current.clone(),
                Some(current) => format!("{current}\n{note}"),
                None => note,
            };
            self.work_order_items
                .persist(WorkOrderItem { observation: Some(observation), ..item.clone() })
                .await
                .map_err(database_error)?;
        }

        // One new order for the unmatched non-conformities.
        let mut new_items = std::collections::HashMap::new();
        if !unmatched.is_empty() {
            let order = self
                .work_orders
                .persist(WorkOrder {
                    id: None,
                    uuid: None,
                    tenant_id: saved.tenant_id,
                    vehicle_id: saved.vehicle_id,
                    opened_at: None,
                    odometer_km: odometer,
                    origin: WorkOrderOrigin::Inspection,
                    checklist_run_id: None,
                    maintenance_plan_id: None,
                    preventive_plan_id: None,
                    service_type: None,
                    description: format!("{model}: {} new non-conforming item(s)", unmatched.len()),
                    responsible: None,
                    status: WorkOrderStatus::Open,
                    observation: saved.observation.clone(),
                    external_service: false,
                    supplier: None,
                    invoice_number: None,
                    invoice_value_cents: None,
                    invoice_date: None,
                    concluded_at: None,
                    created_at: None,
                    created_by: None,
                    updated_at: None,
                    updated_by: None,
                })
                .await
                .map_err(database_error)?;
            let order_id = WorkOrderEntityMapper::from_active_model(order).id.unwrap_or_default();
            for index in &unmatched {
                let answer = &answers[*index];
                let item = self
                    .work_order_items
                    .persist(WorkOrderItem {
                        id: None,
                        uuid: None,
                        tenant_id: saved.tenant_id,
                        work_order_id: order_id,
                        description: answer.description.trim().to_string(),
                        item_type: Some("TechnicalInspection".to_string()),
                        status: WorkOrderItemStatus::Pending,
                        observation: answer.observation.clone(),
                        resolved_by: None,
                        resolved_at: None,
                        resolution_description: None,
                        purchase_order_id: None,
                        is_purchase_placeholder: false,
                        preventive_plan_id: None,
                        created_at: None,
                        created_by: None,
                        updated_at: None,
                        updated_by: None,
                    })
                    .await
                    .map_err(database_error)?;
                new_items.insert(*index, WorkOrderItemEntityMapper::from_active_model(item).id);
            }
        }

        let mut saved_items = Vec::with_capacity(answers.len());
        for (index, answer) in answers.into_iter().enumerate() {
            let work_order_item_id = matched[index].as_ref().and_then(|i| i.id).or_else(|| new_items.get(&index).copied().flatten());
            let entity = self
                .items
                .persist(TechnicalInspectionItem {
                    id: None,
                    uuid: None,
                    tenant_id: saved.tenant_id,
                    technical_inspection_id: inspection_id,
                    description: answer.description.trim().to_string(),
                    conforming: answer.conforming,
                    observation: answer.observation,
                    work_order_item_id,
                    created_at: None,
                    created_by: None,
                    updated_at: None,
                    updated_by: None,
                })
                .await
                .map_err(database_error)?;
            saved_items.push(TechnicalInspectionItemEntityMapper::from_active_model(entity));
        }

        self.discharge_alerts(&saved, vehicle.odometer_km).await?;
        Ok((saved, saved_items, !unmatched.is_empty()))
    }

    /// `TRM-325`: the inspection satisfies the pending alerts that name its model.
    async fn discharge_alerts(&self, inspection: &TechnicalInspection, current_km: Option<f64>) -> Result<(), BusinessError> {
        let model = normalise(&inspection.inspection_model);
        for plan in self.plans.find_by_vehicle(inspection.vehicle_id).await.map_err(database_error)? {
            let plan = PreventivePlanEntityMapper::from_model(plan);
            let alerts = self.alerts.find_by_plan(plan.id.unwrap_or_default()).await.map_err(database_error)?;
            let alerts = PreventivePlanAlertEntityMapper::from_models(alerts);
            for alert in pending_alerts(&plan, alerts, current_km) {
                if normalise(&alert.inspection_model) == model {
                    let key = cycle_key(&plan);
                    self.alerts
                        .persist(crate::domain::preventive_plan_alert::PreventivePlanAlert {
                            discharged_cycle: Some(key),
                            ..alert
                        })
                        .await
                        .map_err(database_error)?;
                }
            }
        }
        Ok(())
    }

    pub async fn find_by_uuid(
        &self,
        uuid: String,
    ) -> Result<(TechnicalInspection, Vec<TechnicalInspectionItem>), BusinessError> {
        let inspection = self
            .inspections
            .find_by_uuid(uuid)
            .await
            .map_err(database_error)?
            .map(TechnicalInspectionEntityMapper::from_model)
            .ok_or_else(|| BusinessError::new(INSPECTION_NOT_FOUND.to_string()))?;
        let items = self.items.find_by_inspection(inspection.id.unwrap_or_default()).await.map_err(database_error)?;
        Ok((inspection, TechnicalInspectionItemEntityMapper::from_models(items)))
    }

    /// The work-order items an inspection's answers point to, so callers can
    /// name every work order it touched.
    pub async fn work_order_uuids(&self, items: &[TechnicalInspectionItem]) -> Result<Vec<String>, BusinessError> {
        let mut uuids = Vec::new();
        for id in items.iter().filter_map(|i| i.work_order_item_id) {
            let Some(item) = self.work_order_items.find_by_id(id).await.map_err(database_error)? else { continue };
            if let Some(order) = self.work_orders.find_by_id(item.work_order_id).await.map_err(database_error)? {
                let uuid = crate::commons::functions::bytes_para_string(order.uuid);
                if !uuids.contains(&uuid) {
                    uuids.push(uuid);
                }
            }
        }
        Ok(uuids)
    }
}

fn normalise(text: &str) -> String {
    crate::commons::functions::normalize_name(text)
}

/// `TRM-333`: the normalised description with the articles of the source
/// language left out, so "Troca do óleo" and "troca oleo" name one defect.
fn defect_key(text: &str) -> String {
    let stop = ["a", "as", "o", "os", "de", "da", "das", "do", "dos"];
    normalise(text).split(' ').filter(|w| !w.is_empty() && !stop.contains(w)).collect::<Vec<_>>().join(" ")
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[TechnicalInspectionUseCase] {}", msg);
    BusinessError::new(msg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_defect_is_named_the_same_despite_accents_case_and_articles() {
        assert_eq!(defect_key("Troca do Óleo"), defect_key("  troca oleo "));
        assert_eq!(defect_key("Folga nas cruzetas"), "folga nas cruzetas");
        assert_ne!(defect_key("Freio dianteiro"), defect_key("Freio traseiro"));
        assert_eq!(defect_key("de da"), "");
    }
}
