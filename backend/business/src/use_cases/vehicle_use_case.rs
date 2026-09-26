use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::vehicle::{Vehicle, VehicleEntityMapper};
use crate::gateway::vehicle_gateway::VehicleGateway;
use sea_orm::{DbErr, SqlErr};

/// EPIC-FO-01-S06 (HRMS-925): the rejection a caller can act on. Matched by
/// `vehicle_endpoint` to answer 409 instead of a generic 400, the same way
/// `UserUseCase::change_password` names its own refusal.
pub const DUPLICATE_PLATE: &str = "A vehicle with this plate is already registered";

/// EPIC-FO-02-S01 (HRMS-926): another vehicle already reports through this
/// tracking device. Only the platform administrator links devices, so this
/// discloses nothing to a tenant.
pub const DUPLICATE_TRACKER_DEVICE: &str = "This tracking device is already linked to a vehicle";

/// DEF-FO-02: two concurrent writes can both pass the pre-checks; the unique
/// index stops the second, and that is still a duplicate, not a malformed
/// request. The index name says which rule was broken (the UUID is generated,
/// so it never collides).
fn duplicate_error(error: &DbErr) -> Option<BusinessError> {
    match error.sql_err() {
        Some(SqlErr::UniqueConstraintViolation(message)) => {
            let rule = if message.contains("uq_vehicle_tracker_device") {
                DUPLICATE_TRACKER_DEVICE
            } else {
                DUPLICATE_PLATE
            };
            Some(BusinessError::new(rule.to_string()))
        }
        _ => None,
    }
}

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
        log::info!(
            "[VehicleUseCase::create] Executing for plate: {}",
            vehicle.plate
        );

        let vehicle = Self::validated(vehicle)?;
        // The tenant the row will actually be written to: `enforce_tenant`
        // (D-06) replaces the payload's with a tenant-bound caller's own.
        let owning_tenant = match entity::audit::tenant_scope() {
            entity::audit::TenantScope::Tenant(id) => Some(id),
            _ => vehicle.tenant_id,
        };
        self.reject_duplicate_plate(&vehicle.plate, owning_tenant, None)
            .await?;
        self.reject_linked_device(vehicle.tracker_device_id, None).await?;

        let entity = self.gateway.persist(vehicle).await.map_err(|e| {
            if let Some(duplicate) = duplicate_error(&e) {
                return duplicate;
            }
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
        self.reject_duplicate_plate(&vehicle.plate, existing.tenant_id, Some(id))
            .await?;
        self.reject_linked_device(vehicle.tracker_device_id, Some(id)).await?;

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
            // Who may change it is the endpoint's rule (HRMS-926): only the
            // platform administrator; for anyone else it arrives unchanged.
            tracker_device_id: vehicle.tracker_device_id,
            prefix: vehicle.prefix,
            vehicle_type: vehicle.vehicle_type,
            odometer_km: vehicle.odometer_km,
            wheel_type: vehicle.wheel_type,
            spare_tire_count: vehicle.spare_tire_count,
            spare_tire_type: vehicle.spare_tire_type,
            spare_tire_notes: vehicle.spare_tire_notes,
            garage_tag: vehicle.garage_tag,
            garage_tag_origin: vehicle.garage_tag_origin,
            created_at: existing.created_at,
            created_by: existing.created_by,
            updated_at: None,
            updated_by: vehicle.updated_by,
        };

        let entity = self.gateway.persist(updated).await.map_err(|e| {
            if let Some(duplicate) = duplicate_error(&e) {
                return duplicate;
            }
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
        Ok(Vehicle {
            plate,
            model,
            ..vehicle
        })
    }

    /// The lookup is tenant-scoped (`VehicleGateway::find_by_plate`), so this
    /// can only ever report a collision inside the caller's own fleet.
    async fn reject_duplicate_plate(
        &self,
        plate: &str,
        tenant_id: Option<i64>,
        editing: Option<i64>,
    ) -> Result<(), BusinessError> {
        let existing = self
            .gateway
            .find_by_plate(plate, tenant_id)
            .await
            .map_err(|e| {
                let msg = format!("Database error: {}", e);
                log::error!("[VehicleUseCase::reject_duplicate_plate] {}", msg);
                BusinessError::new(msg)
            })?;

        match existing {
            Some(model) if Some(model.id) != editing => {
                log::error!(
                    "[VehicleUseCase::reject_duplicate_plate] Plate already registered: {}",
                    plate
                );
                Err(BusinessError::new(DUPLICATE_PLATE.to_string()))
            }
            _ => Ok(()),
        }
    }

    /// HRMS-926: one device reports for one vehicle, platform-wide. The
    /// lookup runs in the caller's scope; only an unrestricted caller may
    /// link a device (the endpoint's rule), and for that caller the scope is
    /// the whole platform, which is exactly the uniqueness wanted.
    async fn reject_linked_device(
        &self,
        device_id: Option<i64>,
        editing: Option<i64>,
    ) -> Result<(), BusinessError> {
        let Some(device_id) = device_id else {
            return Ok(());
        };
        let existing = self
            .gateway
            .find_by_tracker_device(device_id)
            .await
            .map_err(|e| BusinessError::new(format!("Database error: {}", e)))?;
        match existing {
            Some(model) if Some(model.id) != editing => {
                Err(BusinessError::new(DUPLICATE_TRACKER_DEVICE.to_string()))
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
                log::error!(
                    "[VehicleUseCase::find_by_id] Vehicle not found with id: {}",
                    id
                );
                Err(BusinessError::new("Vehicle not found".to_string()))
            }
        }
    }

    /// HRMS-204/AD-010: the UUID is a vehicle's public identifier; the numeric
    /// id stays the database key. A vehicle owned by another tenant is simply
    /// absent from this read (`tenant_select`), which is what lets the endpoint
    /// answer 404 rather than 403 (PD-034).
    pub async fn find_by_uuid(&self, uuid: String) -> Result<Vehicle, BusinessError> {
        log::info!(
            "[VehicleUseCase::find_by_uuid] Executing for uuid: {}",
            uuid
        );

        let entity = self.gateway.find_by_uuid(uuid.clone()).await.map_err(|e| {
            let msg = format!("Database error: {}", e);
            log::error!("[VehicleUseCase::find_by_uuid] {}", msg);
            BusinessError::new(msg)
        })?;

        match entity {
            Some(model) => Ok(VehicleEntityMapper::from_model(model)),
            None => {
                log::error!(
                    "[VehicleUseCase::find_by_uuid] Vehicle not found with uuid: {}",
                    uuid
                );
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
        let (entities, total) = self
            .gateway
            .find_page(page, page_size, search)
            .await
            .map_err(|e| {
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
