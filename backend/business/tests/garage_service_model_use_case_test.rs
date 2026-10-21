use business::domain::business_error::BusinessError;
use business::domain::enums::{GarageServiceGroup, VehicleStatus};
use business::domain::garage_service_model::{GarageServiceApplicability, GarageServiceModel};
use business::domain::vehicle::Vehicle;
use business::gateway::garage_service_model_gateway::GarageServiceModelStore;
use business::gateway::vehicle_gateway::VehicleUuidLookupStore;
use business::use_cases::garage_service_model_use_case::{
    DUPLICATE_NAME, GarageServiceModelUseCase, INVALID_APPLICABILITY, NAME_REQUIRED,
    SERVICE_NOT_FOUND,
};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryGarageServiceStore {
    services: Mutex<Vec<GarageServiceModel>>,
}

#[async_trait]
impl GarageServiceModelStore for MemoryGarageServiceStore {
    async fn save(
        &self,
        mut model: GarageServiceModel,
    ) -> Result<GarageServiceModel, BusinessError> {
        let mut services = self
            .services
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        match model.id {
            Some(id) => {
                if let Some(existing) = services.iter_mut().find(|existing| existing.id == Some(id))
                {
                    *existing = model.clone();
                } else {
                    services.push(model.clone());
                }
            }
            None => {
                let id = services.len() as i64 + 1;
                model.id = Some(id);
                model.uuid = Some(format!("garage-service-{id}"));
                services.push(model.clone());
            }
        }
        Ok(model)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<GarageServiceModel>, BusinessError> {
        let services = self
            .services
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(services
            .iter()
            .find(|service| service.id == Some(id))
            .cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<GarageServiceModel>, BusinessError> {
        let services = self
            .services
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(services
            .iter()
            .find(|service| service.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn find_by_key(
        &self,
        name_key: &str,
        tenant_id: Option<i64>,
    ) -> Result<Option<GarageServiceModel>, BusinessError> {
        let services = self
            .services
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(services
            .iter()
            .find(|service| service.name_key == name_key && service.tenant_id == tenant_id)
            .cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<GarageServiceModel>, u64), BusinessError> {
        let services = self
            .services
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let total = services.len() as u64;
        let start = (page * page_size) as usize;
        Ok((
            services
                .iter()
                .skip(start)
                .take(page_size as usize)
                .cloned()
                .collect(),
            total,
        ))
    }
}

struct MemoryVehicleUuidStore {
    vehicles: Vec<Vehicle>,
}

#[async_trait]
impl VehicleUuidLookupStore for MemoryVehicleUuidStore {
    async fn get_by_uuid(&self, uuid: String) -> Result<Option<Vehicle>, BusinessError> {
        Ok(self
            .vehicles
            .iter()
            .find(|vehicle| vehicle.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }
}

fn vehicle(uuid: &str, tenant_id: Option<i64>) -> Vehicle {
    Vehicle {
        id: Some(10),
        uuid: Some(uuid.into()),
        tenant_id,
        plate: "ABC1D23".into(),
        model: "Truck".into(),
        status: VehicleStatus::Active,
        tracker_device_id: None,
        prefix: None,
        vehicle_type: None,
        odometer_km: None,
        wheel_type: None,
        spare_tire_count: None,
        spare_tire_type: None,
        spare_tire_notes: None,
        tank_capacity_liters: None,
        reference_km_per_liter: None,
        garage_tag: None,
        garage_tag_origin: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn new_service(name: &str) -> GarageServiceModel {
    GarageServiceModel {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        name: name.into(),
        name_key: String::new(),
        display_order: 1,
        active: true,
        service_group: GarageServiceGroup::External,
        required_for_departure: true,
        governed_by_tank: false,
        applicability: GarageServiceApplicability::All,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn new_use_case(
    vehicles: Vec<Vehicle>,
) -> GarageServiceModelUseCase<MemoryGarageServiceStore, MemoryVehicleUuidStore> {
    GarageServiceModelUseCase::new(
        MemoryGarageServiceStore::default(),
        MemoryVehicleUuidStore { vehicles },
    )
}

#[tokio::test]
async fn garage_service_use_case_normalizes_and_pages_through_its_store() {
    let use_case = new_use_case(vec![]);
    let created = use_case
        .create(new_service("  Oil check  "))
        .await
        .expect("service is created");
    let found = use_case
        .find_by_uuid(created.uuid.clone().expect("store assigned uuid"))
        .await
        .expect("service is found");
    let (page, total) = use_case.find_page(0, 10).await.expect("page is listed");

    assert_eq!(created.name, "Oil check");
    assert_eq!(created.name_key, "oil check");
    assert_eq!(found.id, created.id);
    assert_eq!(total, 1);
    assert_eq!(page.len(), 1);
}

#[tokio::test]
async fn garage_service_use_case_rejects_duplicate_names() {
    let use_case = new_use_case(vec![]);
    use_case
        .create(new_service("Oil check"))
        .await
        .expect("first service is created");

    let error = use_case
        .create(new_service("OIL CHECK"))
        .await
        .expect_err("normalized name is unique");

    assert_eq!(error.message, DUPLICATE_NAME);
}

#[tokio::test]
async fn garage_service_vehicle_applicability_requires_a_same_tenant_vehicle() {
    let vehicle_uuid = "684db325-63cb-470a-aa09-45811dd1904a";
    let use_case = new_use_case(vec![vehicle(vehicle_uuid, Some(42))]);
    let mut service = new_service("Oil check");
    service.applicability = GarageServiceApplicability::Vehicles(vec![vehicle_uuid.into()]);

    let created = use_case
        .create(service)
        .await
        .expect("same-tenant vehicle is accepted");

    assert_eq!(
        created.applicability,
        GarageServiceApplicability::Vehicles(vec![vehicle_uuid.into()])
    );

    let other_tenant = new_use_case(vec![vehicle(vehicle_uuid, Some(99))]);
    let mut service = new_service("Other service");
    service.applicability = GarageServiceApplicability::Vehicles(vec![vehicle_uuid.into()]);
    let error = other_tenant
        .create(service)
        .await
        .expect_err("cross-tenant vehicle is rejected");
    assert_eq!(error.message, INVALID_APPLICABILITY);
}

#[tokio::test]
async fn garage_service_names_and_vehicle_type_applicability_are_normalized() {
    let use_case = new_use_case(vec![]);
    assert_eq!(
        use_case
            .create(new_service("  "))
            .await
            .expect_err("service name is required")
            .message,
        NAME_REQUIRED
    );

    let mut service = new_service("Engine inspection");
    service.applicability = GarageServiceApplicability::VehicleTypes(vec![
        "  Van ".into(),
        "van".into(),
        "Truck".into(),
    ]);
    let created = use_case
        .create(service)
        .await
        .expect("valid vehicle types are normalized and deduplicated");
    assert_eq!(created.name, "Engine inspection");
    assert_eq!(
        created.applicability,
        GarageServiceApplicability::VehicleTypes(vec!["Van".into(), "Truck".into()])
    );

    let mut empty_types = new_service("Empty applicability");
    empty_types.applicability = GarageServiceApplicability::VehicleTypes(Vec::new());
    assert_eq!(
        use_case
            .create(empty_types)
            .await
            .expect_err("vehicle type applicability cannot be empty")
            .message,
        INVALID_APPLICABILITY
    );
}

#[tokio::test]
async fn garage_service_update_keeps_identity_and_reports_missing_records() {
    let use_case = new_use_case(vec![]);
    let created = use_case
        .create(new_service("Oil check"))
        .await
        .expect("service is created");
    let mut edited = created.clone();
    edited.name = "  Oil and filter  ".into();

    let updated = use_case
        .update(edited)
        .await
        .expect("service can be renamed");
    assert_eq!(updated.id, created.id);
    assert_eq!(updated.uuid, created.uuid);
    assert_eq!(updated.name, "Oil and filter");
    assert_eq!(updated.name_key, "oil and filter");

    assert_eq!(
        use_case
            .find_by_uuid("missing".into())
            .await
            .expect_err("unknown service UUID is reported")
            .message,
        SERVICE_NOT_FOUND
    );
}

#[tokio::test]
async fn vehicle_applicability_rejects_missing_tenant_invalid_uuid_and_empty_list() {
    let vehicle_uuid = "684db325-63cb-470a-aa09-45811dd1904a";
    let cases = [
        (None, vec![vehicle_uuid.into()]),
        (Some(42), vec!["not-a-uuid".into()]),
        (Some(42), Vec::new()),
    ];
    for (tenant_id, uuids) in cases {
        let mut service = new_service("Vehicle-specific service");
        service.tenant_id = tenant_id;
        service.applicability = GarageServiceApplicability::Vehicles(uuids);
        assert_eq!(
            new_use_case(vec![vehicle(vehicle_uuid, Some(42))])
                .create(service)
                .await
                .expect_err("invalid vehicle-specific scope is rejected")
                .message,
            INVALID_APPLICABILITY
        );
    }
}
