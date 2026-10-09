use crate::domain::business_error::BusinessError;
use crate::domain::enums::Role;
use crate::domain::extra_trip::ExtraTrip;
use crate::gateway::customer_gateway::CustomerStore;
use crate::gateway::extra_trip_gateway::{ExtraTripStore, ExtraTripStoreError};
use crate::gateway::user_gateway::UserLookupStore;
use crate::gateway::vehicle_gateway::VehicleLookupStore;
use crate::use_cases::extra_trip_import::{
    ExtraTripImportError, TripImportBatch, TripImportRejection, TripRejectionReason,
};
use crate::use_cases::reference_import::{ImportOutcome, find_duplicate_keys};

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
    gateway: Box<dyn ExtraTripStore>,
    users: Box<dyn UserLookupStore>,
    vehicles: Box<dyn VehicleLookupStore>,
    customers: Box<dyn CustomerStore>,
}

impl ExtraTripUseCase {
    pub fn new<G, U, V, C>(gateway: G, users: U, vehicles: V, customers: C) -> Self
    where
        G: ExtraTripStore + 'static,
        U: UserLookupStore + 'static,
        V: VehicleLookupStore + 'static,
        C: CustomerStore + 'static,
    {
        Self {
            gateway: Box::new(gateway),
            users: Box::new(users),
            vehicles: Box::new(vehicles),
            customers: Box::new(customers),
        }
    }

    pub async fn create(&self, trip: ExtraTrip) -> Result<ExtraTrip, BusinessError> {
        let trip = self.validated(trip).await?;
        self.reject_duplicate(&trip.order_code, trip.trip_date, trip.tenant_id, None)
            .await?;
        self.gateway.save(trip).await.map_err(store_error)
    }

    pub async fn update(&self, id: i64, trip: ExtraTrip) -> Result<ExtraTrip, BusinessError> {
        let existing = self.find_by_id(id).await?;
        let trip = self.validated(trip).await?;
        self.reject_duplicate(
            &trip.order_code,
            trip.trip_date,
            existing.tenant_id,
            Some(id),
        )
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

        self.gateway.save(updated).await.map_err(store_error)
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
            let customer = self.customers.get_by_id(customer_id).await?;
            let belongs = customer.is_some_and(|c| c.tenant_id == trip.tenant_id);
            if !belongs {
                return Err(BusinessError::new(NOT_A_TENANT_CUSTOMER.to_string()));
            }
        }

        for driver_id in [trip.driver_id, trip.second_driver_id]
            .into_iter()
            .flatten()
        {
            let driver = self.users.get_by_id(driver_id).await?;
            let is_valid_driver = driver.is_some_and(|user| {
                user.role == Role::Driver && user.enabled && user.tenant_id == trip.tenant_id
            });
            if !is_valid_driver {
                return Err(BusinessError::new(NOT_A_DRIVER.to_string()));
            }
        }

        if let Some(vehicle_id) = trip.vehicle_id {
            let vehicle = self.vehicles.get_by_id(vehicle_id).await?;
            let belongs = vehicle.is_some_and(|v| v.tenant_id == trip.tenant_id);
            if !belongs {
                return Err(BusinessError::new(NOT_A_TENANT_VEHICLE.to_string()));
            }
        }

        Ok(ExtraTrip { order_code, ..trip })
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
            .await?;
        match existing {
            Some(trip) if trip.id != editing => Err(BusinessError::new(DUPLICATE_TRIP.to_string())),
            _ => Ok(()),
        }
    }

    pub async fn find_by_id(&self, id: i64) -> Result<ExtraTrip, BusinessError> {
        match self.gateway.find_by_id(id).await? {
            Some(trip) => Ok(trip),
            None => Err(BusinessError::new("Trip not found".to_string())),
        }
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<ExtraTrip, BusinessError> {
        match self.gateway.find_by_uuid(uuid).await? {
            Some(trip) => Ok(trip),
            None => Err(BusinessError::new("Trip not found".to_string())),
        }
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<ExtraTrip>, u64), BusinessError> {
        self.gateway.find_page(page, page_size, search).await
    }

    /// `EPIC-SC-03-S02` (`HRMS-608`, `PD-027`): tudo ou nada -- see
    /// `extra_trip_import`. Every row runs the same relation checks
    /// `create` runs (`validated`), so a driver, vehicle or customer outside
    /// the row's own tenant is rejected the same way whether the trip
    /// arrives one at a time or by the hundred.
    pub async fn import(
        &self,
        rows: Vec<ExtraTrip>,
    ) -> Result<ImportOutcome, ExtraTripImportError> {
        self.import_batch(Self::batch_of(rows)).await
    }

    /// The rows of `rows`, numbered by position, with no rejection known yet.
    fn batch_of(rows: Vec<ExtraTrip>) -> TripImportBatch {
        TripImportBatch {
            rows: rows.into_iter().enumerate().collect(),
            rejections: Vec::new(),
        }
    }

    /// `import`, for a batch whose rows keep their original positions and which may
    /// already carry rejections (see `TripImportBatch`).
    pub async fn import_batch(
        &self,
        batch: TripImportBatch,
    ) -> Result<ImportOutcome, ExtraTripImportError> {
        let prepared = self.prepare_import(batch).await?;
        let rows = prepared.into_iter().map(|(_, trip)| trip).collect();
        // PD-027 all-or-nothing: validation alone cannot promise it, so the
        // write itself is one transaction -- `CityUseCase::import`'s own
        // reasoning.
        let (created, updated) = self.gateway.import(rows).await.map_err(|error| {
            ExtraTripImportError::unavailable("ExtraTripUseCase::import", error.message)
        })?;
        Ok(ImportOutcome { created, updated })
    }

    /// `HRMS-608`: what `import` would do with this batch, **without writing
    /// anything**. It runs the same checks, so a batch that would be rejected is
    /// rejected here with the same rows named; a batch that would be accepted
    /// says, row by row, whether it creates a trip or updates one. Advisory: a
    /// trip recorded between the preview and the import can change an answer.
    pub async fn preview(
        &self,
        rows: Vec<ExtraTrip>,
    ) -> Result<Vec<TripImportPlan>, ExtraTripImportError> {
        self.preview_batch(Self::batch_of(rows)).await
    }

    /// `preview`, for a batch that keeps original positions (see `import_batch`).
    pub async fn preview_batch(
        &self,
        batch: TripImportBatch,
    ) -> Result<Vec<TripImportPlan>, ExtraTripImportError> {
        let prepared = self.prepare_import(batch).await?;
        let mut plan = Vec::with_capacity(prepared.len());
        for (row, trip) in prepared {
            let existing = self
                .gateway
                .find_by_code_and_date(&trip.order_code, trip.trip_date, trip.tenant_id)
                .await
                .map_err(|e| ExtraTripImportError::unavailable("ExtraTripUseCase::preview", e.message))?;
            plan.push(TripImportPlan {
                row,
                order_code: trip.order_code,
                trip_date: trip.trip_date,
                update: existing.is_some(),
            });
        }
        Ok(plan)
    }

    /// The checks `import` and `preview` share: every row is validated first
    /// and every rejection is collected, so the caller sees all of them.
    async fn prepare_import(
        &self,
        batch: TripImportBatch,
    ) -> Result<Vec<(usize, ExtraTrip)>, ExtraTripImportError> {
        let mut prepared = Vec::with_capacity(batch.rows.len());
        let mut rejections = batch.rejections;

        for (index, row) in batch.rows {
            if row.order_code.trim().is_empty() {
                rejections.push(TripImportRejection::new(
                    index,
                    TripRejectionReason::OrderCodeRequired,
                ));
                continue;
            }
            match self.validated(row).await {
                Ok(row) => prepared.push((index, row)),
                Err(e) => {
                    let reason = match e.message.as_str() {
                        NOT_A_DRIVER => TripRejectionReason::NotADriver,
                        NOT_A_TENANT_VEHICLE => TripRejectionReason::NotATenantVehicle,
                        NOT_A_TENANT_CUSTOMER => TripRejectionReason::NotATenantCustomer,
                        _ => {
                            return Err(ExtraTripImportError::unavailable(
                                "ExtraTripUseCase::import",
                                e.message,
                            ));
                        }
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
        Ok(prepared)
    }
}

/// One row of a previewed import: where it sits in the file, which trip it names
/// and whether applying it would update an existing trip (otherwise it creates one).
#[derive(Debug, Clone, PartialEq)]
pub struct TripImportPlan {
    pub row: usize,
    pub order_code: String,
    pub trip_date: chrono::NaiveDate,
    pub update: bool,
}

fn store_error(error: ExtraTripStoreError) -> BusinessError {
    match error {
        ExtraTripStoreError::Duplicate => BusinessError::new(DUPLICATE_TRIP.to_string()),
        ExtraTripStoreError::Storage(error) => error,
    }
}
