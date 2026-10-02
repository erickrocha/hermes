use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::normalize_name;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::garage_service_model::{GarageServiceApplicability, GarageServiceModel, GarageServiceModelEntityMapper};
use crate::gateway::garage_service_model_gateway::GarageServiceModelGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use sea_orm::DbErr;
use std::collections::HashSet;
use uuid::Uuid;

pub const NAME_REQUIRED: &str = "A garage service needs a name";
pub const DUPLICATE_NAME: &str = "This tenant already has a garage service with this name";
pub const SERVICE_NOT_FOUND: &str = "Garage service not found";
pub const INVALID_APPLICABILITY: &str = "Garage service applicability must contain valid values from this tenant";

/// `EPIC-GA-01-S01` (`HRMS-956`): the tenant's catalogue of garage services.
pub struct GarageServiceModelUseCase {
    gateway: GarageServiceModelGateway,
    vehicles: VehicleGateway,
}

impl GarageServiceModelUseCase {
    pub fn new(gateway: GarageServiceModelGateway, vehicles: VehicleGateway) -> Self {
        Self { gateway, vehicles }
    }

    pub async fn create(&self, model: GarageServiceModel) -> Result<GarageServiceModel, BusinessError> {
        let model = self.validated(model, None).await?;
        let entity = self.gateway.persist(model).await.map_err(database_error)?;
        Ok(GarageServiceModelEntityMapper::from_active_model(entity))
    }

    /// Replaces a service's name, order, active flag, group and required flag.
    /// A rename is judged by its new normalised name against the tenant's others.
    pub async fn update(&self, edited: GarageServiceModel) -> Result<GarageServiceModel, BusinessError> {
        let current = self.find_by_uuid(edited.uuid.clone().unwrap_or_default()).await?;
        let model = self.validated(GarageServiceModel { id: current.id, tenant_id: current.tenant_id, ..edited }, current.id).await?;
        let entity = self.gateway.persist(model).await.map_err(database_error)?;
        Ok(GarageServiceModelEntityMapper::from_active_model(entity))
    }

    async fn validated(&self, model: GarageServiceModel, editing: Option<i64>) -> Result<GarageServiceModel, BusinessError> {
        let name = model.name.trim().to_string();
        let name_key = normalize_name(&name);
        if name_key.is_empty() {
            return Err(BusinessError::new(NAME_REQUIRED.to_string()));
        }
        let existing = self.gateway.find_by_key(&name_key, model.tenant_id).await.map_err(database_error)?;
        if existing.is_some_and(|e| Some(e.id) != editing) {
            return Err(BusinessError::new(DUPLICATE_NAME.to_string()));
        }
        let applicability = self.validate_applicability(model.applicability, model.tenant_id).await?;
        Ok(GarageServiceModel { name, name_key, applicability, ..model })
    }

    async fn validate_applicability(
        &self,
        applicability: GarageServiceApplicability,
        tenant_id: Option<i64>,
    ) -> Result<GarageServiceApplicability, BusinessError> {
        match applicability {
            GarageServiceApplicability::All => Ok(GarageServiceApplicability::All),
            GarageServiceApplicability::VehicleTypes(types) => {
                let mut seen = HashSet::new();
                let mut normalized_types = Vec::new();
                for vehicle_type in types {
                    let vehicle_type = vehicle_type.trim().to_string();
                    let key = normalize_name(&vehicle_type);
                    if key.is_empty() {
                        return Err(BusinessError::new(INVALID_APPLICABILITY.to_string()));
                    }
                    if seen.insert(key) {
                        normalized_types.push(vehicle_type);
                    }
                }
                if normalized_types.is_empty() {
                    return Err(BusinessError::new(INVALID_APPLICABILITY.to_string()));
                }
                Ok(GarageServiceApplicability::VehicleTypes(normalized_types))
            }
            GarageServiceApplicability::Vehicles(vehicles) => {
                let Some(tenant_id) = tenant_id else {
                    return Err(BusinessError::new(INVALID_APPLICABILITY.to_string()));
                };
                let mut seen = HashSet::new();
                let mut vehicle_uuids = Vec::new();
                for vehicle_uuid in vehicles {
                    let parsed = Uuid::parse_str(&vehicle_uuid)
                        .map_err(|_| BusinessError::new(INVALID_APPLICABILITY.to_string()))?;
                    let vehicle_uuid = parsed.to_string();
                    if !seen.insert(vehicle_uuid.clone()) {
                        continue;
                    }
                    let vehicle = self.vehicles.find_by_uuid(vehicle_uuid.clone()).await.map_err(database_error)?;
                    if !vehicle.is_some_and(|vehicle| vehicle.tenant_id == Some(tenant_id)) {
                        return Err(BusinessError::new(INVALID_APPLICABILITY.to_string()));
                    }
                    vehicle_uuids.push(vehicle_uuid);
                }
                if vehicle_uuids.is_empty() {
                    return Err(BusinessError::new(INVALID_APPLICABILITY.to_string()));
                }
                Ok(GarageServiceApplicability::Vehicles(vehicle_uuids))
            }
        }
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<GarageServiceModel, BusinessError> {
        self.gateway
            .find_by_uuid(uuid)
            .await
            .map_err(database_error)?
            .map(GarageServiceModelEntityMapper::from_model)
            .ok_or_else(|| BusinessError::new(SERVICE_NOT_FOUND.to_string()))
    }

    pub async fn find_page(&self, page: u64, page_size: u64) -> Result<(Vec<GarageServiceModel>, u64), BusinessError> {
        let (rows, total) = self.gateway.find_page(page, page_size).await.map_err(database_error)?;
        Ok((GarageServiceModelEntityMapper::from_models(rows), total))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[GarageServiceModelUseCase] {}", msg);
    BusinessError::new(msg)
}
