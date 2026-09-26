use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::Role;
use crate::domain::extra_trip::{ExtraTrip, ExtraTripEntityMapper};
use crate::gateway::customer_gateway::CustomerGateway;
use crate::gateway::extra_trip_gateway::ExtraTripGateway;
use crate::gateway::user_gateway::UserGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use crate::use_cases::extra_trip_import::{ExtraTripImportError, TripImportRejection, TripRejectionReason};
use crate::use_cases::reference_import::{ImportOutcome, find_duplicate_keys};
use sea_orm::{ActiveModelTrait, DbErr, SqlErr, TransactionTrait};

/// `HRMS-607`/`D-24(d)`: this tenant already has a trip recorded with that
/// order code on that date (`uq_extra_trip_tenant_code_date`). Answered 409.
pub const DUPLICATE_TRIP: &str = "A trip with this order code is already registered for this date";
/// A named driver (either seat) does not name an active `Driver` of the
/// trip's own tenant.
pub const NOT_A_DRIVER: &str = "The person named is not an active driver of this tenant";
/// `vehicle_id` does not name a vehicle of the trip's own tenant.
pub const NOT_A_TENANT_VEHICLE: &str = "The vehicle named does not belong to this tenant";
/// `customer_id` does not name a customer of the trip's own tenant.
pub const NOT_A_TENANT_CUSTOMER: &str = "The customer named does not belong to this tenant";

pub struct ExtraTripUseCase {
    gateway: ExtraTripGateway,
    users: UserGateway,
    vehicles: VehicleGateway,
    customers: CustomerGateway,
}

impl ExtraTripUseCase {
    pub fn new(
        gateway: ExtraTripGateway,
        users: UserGateway,
        vehicles: VehicleGateway,
        customers: CustomerGateway,
    ) -> Self {
        Self {
            gateway,
            users,
            vehicles,
            customers,
        }
    }

    pub async fn create(&self, trip: ExtraTrip) -> Result<ExtraTrip, BusinessError> {
        let trip = self.validated(trip).await?;
        self.reject_duplicate(&trip.order_code, trip.trip_date, trip.tenant_id, None)
            .await?;
        let entity = self.gateway.persist(trip).await.map_err(|e| {
            if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
                BusinessError::new(DUPLICATE_TRIP.to_string())
            } else {
                database_error(e)
            }
        })?;
        Ok(ExtraTripEntityMapper::from_active_model(entity))
    }

    pub async fn update(&self, id: i64, trip: ExtraTrip) -> Result<ExtraTrip, BusinessError> {
        let existing = self.find_by_id(id).await?;
        let trip = self.validated(trip).await?;
        self.reject_duplicate(&trip.order_code, trip.trip_date, existing.tenant_id, Some(id))
            .await?;

        let updated = ExtraTrip {
            id: Some(id),
            uuid: existing.uuid,
            tenant_id: existing.tenant_id,
            created_at: existing.created_at,
            created_by: existing.created_by,
            updated_at: None,
            ..trip
        };

        let entity = self.gateway.persist(updated).await.map_err(|e| {
            if matches!(e.sql_err(), Some(SqlErr::UniqueConstraintViolation(_))) {
                BusinessError::new(DUPLICATE_TRIP.to_string())
            } else {
                database_error(e)
            }
        })?;
        Ok(ExtraTripEntityMapper::from_active_model(entity))
    }

    /// The order code is required; every relation (`customer_id`,
    /// `driver_id`, `second_driver_id`, `vehicle_id`) is optional, validated
    /// only when named -- the same shape `transport_demand_use_case`
    /// treats its own optional preferences.
    async fn validated(&self, trip: ExtraTrip) -> Result<ExtraTrip, BusinessError> {
        let order_code = trip.order_code.trim().to_string();
        if order_code.is_empty() {
            let msg = "Trip order code is required".to_string();
            log::error!("[ExtraTripUseCase::validated] {}", msg);
            return Err(BusinessError::new(msg));
        }

        if let Some(customer_id) = trip.customer_id {
            let customer = self
                .customers
                .find_by_id(customer_id)
                .await
                .map_err(database_error)?;
            let belongs = customer.is_some_and(|c| c.tenant_id == trip.tenant_id);
            if !belongs {
                return Err(BusinessError::new(NOT_A_TENANT_CUSTOMER.to_string()));
            }
        }

        for driver_id in [trip.driver_id, trip.second_driver_id].into_iter().flatten() {
            let driver = self.users.find_by_id(driver_id).await.map_err(database_error)?;
            let is_valid_driver = driver.is_some_and(|user| {
                user.role == Role::Driver.to_string() && user.enabled && user.tenant_id == trip.tenant_id
            });
            if !is_valid_driver {
                return Err(BusinessError::new(NOT_A_DRIVER.to_string()));
            }
        }

        if let Some(vehicle_id) = trip.vehicle_id {
            let vehicle = self
                .vehicles
                .find_by_id(vehicle_id)
                .await
                .map_err(database_error)?;
            let belongs = vehicle.is_some_and(|v| v.tenant_id == trip.tenant_id);
            if !belongs {
                return Err(BusinessError::new(NOT_A_TENANT_VEHICLE.to_string()));
            }
        }

        Ok(ExtraTrip {
            order_code,
            ..trip
        })
    }

    async fn reject_duplicate(
        &self,
        order_code: &str,
        trip_date: sea_orm::prelude::Date,
        tenant_id: Option<i64>,
        editing: Option<i64>,
    ) -> Result<(), BusinessError> {
        let existing = self
            .gateway
            .find_by_code_and_date(order_code, trip_date, tenant_id)
            .await
            .map_err(database_error)?;
        match existing {
            Some(model) if Some(model.id) != editing => Err(BusinessError::new(DUPLICATE_TRIP.to_string())),
            _ => Ok(()),
        }
    }

    pub async fn find_by_id(&self, id: i64) -> Result<ExtraTrip, BusinessError> {
        let entity = self.gateway.find_by_id(id).await.map_err(database_error)?;
        match entity {
            Some(model) => Ok(ExtraTripEntityMapper::from_model(model)),
            None => Err(BusinessError::new("Trip not found".to_string())),
        }
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<ExtraTrip, BusinessError> {
        let entity = self.gateway.find_by_uuid(uuid).await.map_err(database_error)?;
        match entity {
            Some(model) => Ok(ExtraTripEntityMapper::from_model(model)),
            None => Err(BusinessError::new("Trip not found".to_string())),
        }
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<ExtraTrip>, u64), BusinessError> {
        let (entities, total) = self
            .gateway
            .find_page(page, page_size, search)
            .await
            .map_err(database_error)?;
        Ok((ExtraTripEntityMapper::from_models(entities), total))
    }

    /// `EPIC-SC-03-S02` (`HRMS-608`, `PD-027`): tudo ou nada -- see
    /// `extra_trip_import`. Every row runs the same relation checks
    /// `create` runs (`validated`), so a driver, vehicle or customer outside
    /// the row's own tenant is rejected the same way whether the trip
    /// arrives one at a time or by the hundred.
    pub async fn import(&self, rows: Vec<ExtraTrip>) -> Result<ImportOutcome, ExtraTripImportError> {
        let mut prepared = Vec::with_capacity(rows.len());
        let mut rejections = Vec::new();

        for (index, row) in rows.into_iter().enumerate() {
            if row.order_code.trim().is_empty() {
                rejections.push(TripImportRejection::new(index, TripRejectionReason::OrderCodeRequired));
                continue;
            }
            match self.validated(row).await {
                Ok(row) => prepared.push((index, row)),
                Err(e) => {
                    let reason = match e.message.as_str() {
                        NOT_A_DRIVER => TripRejectionReason::NotADriver,
                        NOT_A_TENANT_VEHICLE => TripRejectionReason::NotATenantVehicle,
                        NOT_A_TENANT_CUSTOMER => TripRejectionReason::NotATenantCustomer,
                        _ => return Err(ExtraTripImportError::unavailable("ExtraTripUseCase::import", e.message)),
                    };
                    rejections.push(TripImportRejection::new(index, reason));
                }
            }
        }

        // D-24(d): the identity pair, checked within the file the same way
        // `find_duplicate_keys` already checks city/province imports.
        let keys = prepared
            .iter()
            .map(|(_, t)| (t.tenant_id, t.order_code.clone(), t.trip_date));
        for position in find_duplicate_keys(keys) {
            rejections.push(TripImportRejection::new(
                prepared[position].0,
                TripRejectionReason::DuplicateTripInFile,
            ));
        }

        if !rejections.is_empty() {
            rejections.sort_by_key(|r| r.row);
            return Err(ExtraTripImportError::Rejected(rejections));
        }

        // PD-027 all-or-nothing: validation alone cannot promise it, so the
        // write itself is one transaction -- `CityUseCase::import`'s own
        // reasoning.
        let transaction = self
            .gateway
            .db()
            .begin()
            .await
            .map_err(|e| ExtraTripImportError::unavailable("ExtraTripUseCase::import", e))?;

        let mut outcome = ImportOutcome::default();
        for (_, mut row) in prepared {
            // D-24(d): existing by order code + date, not code alone.
            let existing = self
                .gateway
                .find_by_code_and_date(&row.order_code, row.trip_date, row.tenant_id)
                .await
                .map_err(|e| ExtraTripImportError::unavailable("ExtraTripUseCase::import", e))?;
            match existing {
                Some(found) => {
                    row.id = Some(found.id);
                    outcome.updated += 1;
                }
                None => outcome.created += 1,
            }
            if let Err(e) = ExtraTripEntityMapper::build_active_model(row)
                .save(&transaction)
                .await
            {
                let _ = transaction.rollback().await;
                return Err(ExtraTripImportError::unavailable("ExtraTripUseCase::import", e));
            }
        }

        transaction
            .commit()
            .await
            .map_err(|e| ExtraTripImportError::unavailable("ExtraTripUseCase::import", e))?;
        Ok(outcome)
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[ExtraTripUseCase] {}", msg);
    BusinessError::new(msg)
}
