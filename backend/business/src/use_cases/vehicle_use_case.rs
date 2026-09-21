use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::vehicle::{Vehicle, VehicleEntityMapper};
use crate::gateway::vehicle_gateway::VehicleGateway;

/// EPIC-FO-01-S06 (HRMS-925): the rejection a caller can act on. Matched by
/// `vehicle_endpoint` to answer 409 instead of a generic 400, the same way
/// `UserUseCase::change_password` names its own refusal.
pub const DUPLICATE_PLATE: &str = "A vehicle with this plate is already registered";

pub struct VehicleUseCase {
    gateway: VehicleGateway,
}

impl VehicleUseCase {
    pub fn new(gateway: VehicleGateway) -> Self {
        Self { gateway }
    }

    /// EPIC-FO-01-S06 (HRMS-925, D-23(c)): plates are compared in one
    /// spelling. `uq_vehicle_tenant_plate` would otherwise accept `abc1d23`
    /// and `ABC1D23` as two vehicles -- the same normalise-then-constrain
    /// order HRMS-210 established for a tenant's tax id.
    pub fn normalise_plate(plate: &str) -> String {
        plate.trim().to_uppercase()
    }

    pub async fn create(&self, vehicle: Vehicle) -> Result<Vehicle, BusinessError> {
        log::info!("[VehicleUseCase::create] Executing for plate: {}", vehicle.plate);

        let vehicle = Self::validated(vehicle)?;
        self.reject_duplicate_plate(&vehicle.plate, None).await?;

        let entity = self.gateway.persist(vehicle).await.map_err(|e| {
            let msg = format!("Failed to persist vehicle: {}", e);
            log::error!("[VehicleUseCase::create] {}", msg);
            BusinessError::new(msg)
        })?;

        Ok(VehicleEntityMapper::from_active_model(entity))
    }

    pub async fn update(&self, id: i64, vehicle: Vehicle) -> Result<Vehicle, BusinessError> {
        log::info!("[VehicleUseCase::update] Executing for vehicle id {}", id);

        let existing = self.find_by_id(id).await?;
        let vehicle = Self::validated(vehicle)?;
        self.reject_duplicate_plate(&vehicle.plate, Some(id)).await?;

        let updated = Vehicle {
            id: Some(id),
            uuid: existing.uuid,
            // HRMS-921: a vehicle never changes hands through an edit. The
            // owning tenant is the one already on the row, not one a payload
            // may name.
            tenant_id: existing.tenant_id,
            plate: vehicle.plate,
            model: vehicle.model,
            status: vehicle.status,
            created_at: existing.created_at,
            created_by: existing.created_by,
            updated_at: None,
            updated_by: vehicle.updated_by,
        };

        let entity = self.gateway.persist(updated).await.map_err(|e| {
            let msg = format!("Failed to update vehicle: {}", e);
            log::error!("[VehicleUseCase::update] {}", msg);
            BusinessError::new(msg)
        })?;

        Ok(VehicleEntityMapper::from_active_model(entity))
    }

    /// HRMS-920: a vehicle without a plate or a model is not a fleet record.
    /// The status needs no check here -- it is a `VehicleStatus`, so an
    /// unknown one was already refused at the edge
    /// (`reject_unknown_vehicle_status`) and cannot reach this type.
    fn validated(vehicle: Vehicle) -> Result<Vehicle, BusinessError> {
        let plate = Self::normalise_plate(&vehicle.plate);
        if plate.is_empty() {
            let msg = "Vehicle plate is required".to_string();
            log::error!("[VehicleUseCase::validated] {}", msg);
            return Err(BusinessError::new(msg));
        }
        let model = vehicle.model.trim().to_string();
        if model.is_empty() {
            let msg = "Vehicle model is required".to_string();
            log::error!("[VehicleUseCase::validated] {}", msg);
            return Err(BusinessError::new(msg));
        }
        Ok(Vehicle { plate, model, ..vehicle })
    }

    /// The lookup is tenant-scoped (`VehicleGateway::find_by_plate`), so this
    /// can only ever report a collision inside the caller's own fleet.
    async fn reject_duplicate_plate(&self, plate: &str, editing: Option<i64>) -> Result<(), BusinessError> {
        let existing = self.gateway.find_by_plate(plate).await.map_err(|e| {
            let msg = format!("Database error: {}", e);
            log::error!("[VehicleUseCase::reject_duplicate_plate] {}", msg);
            BusinessError::new(msg)
        })?;

        match existing {
            Some(model) if Some(model.id) != editing => {
                log::error!("[VehicleUseCase::reject_duplicate_plate] Plate already registered: {}", plate);
                Err(BusinessError::new(DUPLICATE_PLATE.to_string()))
            }
            _ => Ok(()),
        }
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Vehicle, BusinessError> {
        log::info!("[VehicleUseCase::find_by_id] Executing for id: {}", id);

        let entity = self.gateway.find_by_id(id).await.map_err(|e| {
            let msg = format!("Database error: {}", e);
            log::error!("[VehicleUseCase::find_by_id] {}", msg);
            BusinessError::new(msg)
        })?;

        match entity {
            Some(model) => Ok(VehicleEntityMapper::from_model(model)),
            None => {
                log::error!("[VehicleUseCase::find_by_id] Vehicle not found with id: {}", id);
                Err(BusinessError::new("Vehicle not found".to_string()))
            }
        }
    }

    /// HRMS-204/AD-010: the UUID is a vehicle's public identifier; the numeric
    /// id stays the database key. A vehicle owned by another tenant is simply
    /// absent from this read (`tenant_select`), which is what lets the endpoint
    /// answer 404 rather than 403 (PD-034).
    pub async fn find_by_uuid(&self, uuid: String) -> Result<Vehicle, BusinessError> {
        log::info!("[VehicleUseCase::find_by_uuid] Executing for uuid: {}", uuid);

        let entity = self.gateway.find_by_uuid(uuid.clone()).await.map_err(|e| {
            let msg = format!("Database error: {}", e);
            log::error!("[VehicleUseCase::find_by_uuid] {}", msg);
            BusinessError::new(msg)
        })?;

        match entity {
            Some(model) => Ok(VehicleEntityMapper::from_model(model)),
            None => {
                log::error!("[VehicleUseCase::find_by_uuid] Vehicle not found with uuid: {}", uuid);
                Err(BusinessError::new("Vehicle not found".to_string()))
            }
        }
    }

    /// PD-028: one page plus the total. The tenant scope comes from the
    /// gateway, not from a filter repeated here.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<Vehicle>, u64), BusinessError> {
        let (entities, total) = self.gateway.find_page(page, page_size, search).await.map_err(|e| {
            let msg = format!("Database error: {}", e);
            log::error!("[VehicleUseCase::find_page] {}", msg);
            BusinessError::new(msg)
        })?;
        Ok((VehicleEntityMapper::from_models(entities), total))
    }

    pub async fn find_all(&self) -> Result<Vec<Vehicle>, BusinessError> {
        log::info!("[VehicleUseCase::find_all] Executing find_all vehicles");

        let entities = self.gateway.find_all().await.map_err(|e| {
            let msg = format!("Database error: {}", e);
            log::error!("[VehicleUseCase::find_all] {}", msg);
            BusinessError::new(msg)
        })?;

        Ok(VehicleEntityMapper::from_models(entities))
    }
}
