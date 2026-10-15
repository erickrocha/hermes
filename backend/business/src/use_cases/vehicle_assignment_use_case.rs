use crate::domain::business_error::BusinessError;
use crate::domain::enums::Role;
use crate::domain::user::User;
use crate::domain::vehicle::Vehicle;
use crate::domain::vehicle_assignment::VehicleAssignment;
use crate::gateway::user_gateway::UserLookupStore;
use crate::gateway::vehicle_assignment_gateway::{
    VehicleAssignmentStore, VehicleAssignmentStoreError,
};
use chrono::Utc;

/// D-23(d): the vehicle already has a live assignment. Answered 409.
pub const VEHICLE_ALREADY_ASSIGNED: &str = "This vehicle already has a live assignment";
/// The vehicle has no live assignment to read or end. Answered 404.
pub const NO_LIVE_ASSIGNMENT: &str = "This vehicle has no live assignment";
/// HRMS-932: the person named is not an active driver of the vehicle's own
/// tenant. One answer for every reason, so it discloses nothing about users
/// outside the caller's tenant. Answered 400.
pub const NOT_A_DRIVER: &str = "The person named is not an active driver of this tenant";

pub struct VehicleAssignmentUseCase<A: VehicleAssignmentStore, U: UserLookupStore> {
    gateway: A,
    users: U,
}

impl<A: VehicleAssignmentStore, U: UserLookupStore> VehicleAssignmentUseCase<A, U> {
    pub fn new(gateway: A, users: U) -> Self {
        Self { gateway, users }
    }

    /// EPIC-FO-03-S01/S02/S04 (HRMS-931, HRMS-932, HRMS-934). The caller has
    /// already been allowed to administer `vehicle`.
    pub async fn assign(
        &self,
        vehicle: &Vehicle,
        driver_uuid: String,
    ) -> Result<(VehicleAssignment, User), BusinessError> {
        let driver = self
            .users
            .get_by_uuid(driver_uuid)
            .await?
            .filter(|user| {
                user.role == Role::Driver && user.enabled && user.tenant_id == vehicle.tenant_id
            })
            .ok_or_else(|| BusinessError::new(NOT_A_DRIVER.to_string()))?;

        let vehicle_id = persisted_vehicle_id(vehicle)?;
        if self
            .gateway
            .get_live_by_vehicle(vehicle_id)
            .await?
            .is_some()
        {
            return Err(BusinessError::new(VEHICLE_ALREADY_ASSIGNED.to_string()));
        }
        let driver_id = driver
            .id
            .filter(|id| *id > 0)
            .ok_or_else(|| BusinessError::new(NOT_A_DRIVER.to_string()))?;

        let assignment = VehicleAssignment {
            id: None,
            uuid: None,
            tenant_id: vehicle.tenant_id,
            vehicle_id,
            driver_id,
            started_at: Utc::now(),
            ended_at: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        };
        // Two concurrent assignments both pass the check above;
        // `uq_vehicle_assignment_live` stops the second.
        let assignment = self.gateway.save(assignment).await.map_err(store_error)?;
        Ok((assignment, driver))
    }

    /// HRMS-933: a shift handover ends the live assignment; nothing is
    /// deleted, so the history keeps who held the vehicle and when.
    pub async fn end(&self, vehicle: &Vehicle) -> Result<(VehicleAssignment, User), BusinessError> {
        let mut live = self.live(vehicle).await?;
        live.ended_at = Some(Utc::now());
        let ended = self.gateway.save(live).await.map_err(store_error)?;
        let driver = self.driver_of(&ended).await?;
        Ok((ended, driver))
    }

    pub async fn current(
        &self,
        vehicle: &Vehicle,
    ) -> Result<(VehicleAssignment, User), BusinessError> {
        let live = self.live(vehicle).await?;
        let driver = self.driver_of(&live).await?;
        Ok((live, driver))
    }

    /// PD-028.
    // ponytail: one driver lookup per row; a join when pages get long.
    pub async fn history(
        &self,
        vehicle: &Vehicle,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<(VehicleAssignment, User)>, u64), BusinessError> {
        let (rows, total) = self
            .gateway
            .list_by_vehicle(persisted_vehicle_id(vehicle)?, page, page_size)
            .await?;
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let driver = self.driver_of(&row).await?;
            items.push((row, driver));
        }
        Ok((items, total))
    }

    async fn live(&self, vehicle: &Vehicle) -> Result<VehicleAssignment, BusinessError> {
        self.gateway
            .get_live_by_vehicle(persisted_vehicle_id(vehicle)?)
            .await?
            .ok_or_else(|| BusinessError::new(NO_LIVE_ASSIGNMENT.to_string()))
    }

    async fn driver_of(&self, assignment: &VehicleAssignment) -> Result<User, BusinessError> {
        self.users
            .get_by_id(assignment.driver_id)
            .await?
            .ok_or_else(|| BusinessError::new("Assigned driver not found".to_string()))
    }
}

fn persisted_vehicle_id(vehicle: &Vehicle) -> Result<i64, BusinessError> {
    vehicle
        .id
        .filter(|id| *id > 0)
        .ok_or_else(|| BusinessError::new("Vehicle identity is incomplete".to_string()))
}

fn store_error(error: VehicleAssignmentStoreError) -> BusinessError {
    match error {
        VehicleAssignmentStoreError::AlreadyAssigned => {
            BusinessError::new(VEHICLE_ALREADY_ASSIGNED.to_string())
        }
        VehicleAssignmentStoreError::Storage(error) => error,
    }
}
