use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::garage_call::{GarageCall, GarageCallEntityMapper};
use crate::gateway::garage_call_gateway::GarageCallGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use crate::gateway::vehicle_presence_event_gateway::VehiclePresenceEventGateway;
use chrono::NaiveDateTime;
use sea_orm::DbErr;

pub const VEHICLE_NOT_FOUND: &str = "Vehicle not found";
pub const NOT_AWAY: &str = "A vehicle can only be called to base while it is away from base";
pub const NO_ACTIVE_CALL: &str = "This vehicle has no active call to cancel";

/// `EPIC-GA-05-S02` (`HRMS-963`): the manager's manual call to base (`TRM-483…486`).
pub struct GarageCallUseCase {
    calls: GarageCallGateway,
    events: VehiclePresenceEventGateway,
    vehicles: VehicleGateway,
}

impl GarageCallUseCase {
    pub fn new(calls: GarageCallGateway, events: VehiclePresenceEventGateway, vehicles: VehicleGateway) -> Self {
        Self { calls, events, vehicles }
    }

    /// `TRM-483`/`486`: calls an away vehicle to base, stamping the instant. A call
    /// already in force is returned instead of stamped again (`false` = not new).
    pub async fn call(
        &self,
        tenant_id: Option<i64>,
        vehicle_id: i64,
        by_user_id: Option<i64>,
    ) -> Result<(GarageCall, bool), BusinessError> {
        let vehicle = self.vehicles.find_by_id(vehicle_id).await.map_err(database_error)?;
        if !vehicle.is_some_and(|v| v.tenant_id == tenant_id) {
            return Err(BusinessError::new(VEHICLE_NOT_FOUND.to_string()));
        }
        let last_arrival = self.last_arrival(vehicle_id).await?;
        let last_departure = self.last_departure(vehicle_id).await?;
        let away = match (last_arrival, last_departure) {
            (_, None) => false,
            (None, Some(_)) => true,
            (Some(arrival), Some(departure)) => departure > arrival,
        };
        if !away {
            return Err(BusinessError::new(NOT_AWAY.to_string()));
        }
        if let Some(existing) = self.active(vehicle_id).await? {
            return Ok((existing, false));
        }
        let saved = self
            .calls
            .persist(GarageCall {
                id: None,
                uuid: None,
                tenant_id,
                vehicle_id,
                called_at: chrono::Utc::now().naive_utc(),
                called_by_user_id: by_user_id,
                cancelled_at: None,
                created_at: None,
                created_by: None,
                updated_at: None,
                updated_by: None,
            })
            .await
            .map_err(database_error)?;
        Ok((GarageCallEntityMapper::from_active_model(saved), true))
    }

    /// `TRM-486`: a call made in error can be cancelled.
    pub async fn cancel(&self, vehicle_id: i64) -> Result<GarageCall, BusinessError> {
        let call = self
            .active(vehicle_id)
            .await?
            .ok_or_else(|| BusinessError::new(NO_ACTIVE_CALL.to_string()))?;
        let saved = self
            .calls
            .persist(GarageCall { cancelled_at: Some(chrono::Utc::now().naive_utc()), ..call })
            .await
            .map_err(database_error)?;
        Ok(GarageCallEntityMapper::from_active_model(saved))
    }

    /// `TRM-484`: the call in force -- cleared by the vehicle's physical arrival
    /// being later than the call, **never by its tag**, and never needing anyone
    /// to unmark it; or by a cancellation.
    pub async fn active(&self, vehicle_id: i64) -> Result<Option<GarageCall>, BusinessError> {
        let Some(model) = self.calls.find_latest_uncancelled(vehicle_id).await.map_err(database_error)? else {
            return Ok(None);
        };
        let call = GarageCallEntityMapper::from_model(model);
        let arrival = self.last_arrival(vehicle_id).await?;
        Ok(call_in_force(&call, arrival).then_some(call))
    }

    /// Whether the vehicle is away from base by its physical stamps alone (`TRM-484`).
    pub async fn is_away(&self, vehicle_id: i64) -> Result<bool, BusinessError> {
        Ok(match (self.last_arrival(vehicle_id).await?, self.last_departure(vehicle_id).await?) {
            (_, None) => false,
            (None, Some(_)) => true,
            (Some(arrival), Some(departure)) => departure > arrival,
        })
    }

    async fn last_arrival(&self, vehicle_id: i64) -> Result<Option<NaiveDateTime>, BusinessError> {
        Ok(self
            .events
            .find_latest_of_kind(vehicle_id, "Arrival")
            .await
            .map_err(database_error)?
            .map(|m| m.occurred_at.naive_utc()))
    }

    async fn last_departure(&self, vehicle_id: i64) -> Result<Option<NaiveDateTime>, BusinessError> {
        Ok(self
            .events
            .find_latest_of_kind(vehicle_id, "Departure")
            .await
            .map_err(database_error)?
            .map(|m| m.occurred_at.naive_utc()))
    }
}

/// `TRM-484`: in force until the vehicle's physical arrival is later than the call.
pub fn call_in_force(call: &GarageCall, last_arrival: Option<NaiveDateTime>) -> bool {
    call.cancelled_at.is_none() && !last_arrival.is_some_and(|arrival| arrival > call.called_at)
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[GarageCallUseCase] {}", msg);
    BusinessError::new(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(h: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 30).unwrap().and_hms_opt(h, 0, 0).unwrap()
    }

    fn call() -> GarageCall {
        GarageCall {
            id: Some(1),
            uuid: None,
            tenant_id: Some(1),
            vehicle_id: 1,
            called_at: at(10),
            called_by_user_id: None,
            cancelled_at: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
    }

    #[test]
    fn a_call_stays_in_force_until_a_later_physical_arrival_and_only_that() {
        assert!(call_in_force(&call(), None));
        assert!(call_in_force(&call(), Some(at(9))), "an arrival before the call does not clear it");
        assert!(!call_in_force(&call(), Some(at(11))), "the vehicle arrived after the call");
        let cancelled = GarageCall { cancelled_at: Some(at(10)), ..call() };
        assert!(!call_in_force(&cancelled, None));
    }
}
