use crate::AppState;
use business::commons::gateway::Gateway;
use business::domain::business_error::BusinessError;
use business::domain::enums::{PresenceEventKind, PresenceSource};
use business::gateway::garage_cycle_gateway::GarageCycleGateway;
use business::gateway::tenant_gateway::TenantGateway;
use business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway;
use business::gateway::tracking_provider::{TrackingProvider, configured_provider};
use business::gateway::vehicle_gateway::{VehicleGateway, VehicleStore};
use business::gateway::vehicle_presence_event_gateway::VehiclePresenceEventGateway;
use business::use_cases::garage_cycle::GarageCycleSample;
use business::use_cases::garage_cycle_persistence_use_case::GarageCyclePersistenceUseCase;
use business::use_cases::garage_geofence::{
    is_within_base, position_is_fresh_and_valid, transition_for_position,
};
use business::use_cases::vehicle_presence_use_case::VehiclePresenceUseCase;
use chrono::Utc;
use std::collections::HashMap;
use std::time::Duration;

const ACTOR: &str = "garage-geofence";

pub async fn run_presence_poll_once(state: &AppState) {
    let provider = match configured_provider() {
        Ok(provider) => provider,
        Err(error) => {
            log::error!(
                "[{ACTOR}] tracking provider is unavailable: {}",
                error.message
            );
            return;
        }
    };
    let positions = match provider.last_positions().await {
        Ok(positions) => positions,
        Err(error) => {
            log::error!("[{ACTOR}] positions could not be read: {}", error.message);
            return;
        }
    };
    let positions: HashMap<_, _> = positions
        .into_iter()
        .map(|position| (position.device_id, position))
        .collect();

    let db = state.conn.as_ref().clone();
    let tenants =
        match entity::audit::run_as_platform(TenantGateway::new(db.clone()).find_all()).await {
            Ok(tenants) => tenants,
            Err(error) => {
                log::error!("[{ACTOR}] tenants could not be listed: {error}");
                return;
            }
        };

    for tenant in tenants {
        let tenant_id = tenant.id;
        let result = entity::audit::run_for_tenant(tenant_id, ACTOR, async {
            let settings = TenantRuleSettingGateway::new(db.clone())
                .settings_for(Some(tenant_id))
                .await
                .map_err(|error| BusinessError::new(format!("Garage settings error: {error}")))?;
            if settings.garage_base_latitude.is_none() || settings.garage_base_longitude.is_none() {
                return Ok::<usize, BusinessError>(0);
            }

            let vehicles = VehicleGateway::new(db.clone()).list_all().await?;
            let presence = VehiclePresenceUseCase::new(
                VehiclePresenceEventGateway::new(db.clone()),
                VehicleGateway::new(db.clone()),
            );
            let cycles = GarageCyclePersistenceUseCase::new(GarageCycleGateway::new(db.clone()));
            let mut recorded = 0;
            let now = Utc::now();

            for vehicle in vehicles {
                let (Some(vehicle_id), Some(device_id)) = (vehicle.id, vehicle.tracker_device_id)
                else {
                    continue;
                };
                let Some(position) = positions.get(&device_id) else {
                    continue;
                };
                if !position_is_fresh_and_valid(position, Utc::now()) {
                    continue;
                }
                let snapshot = match presence.snapshot(vehicle_id).await {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        log::error!("[{ACTOR}] vehicle {vehicle_id}: {}", error.message);
                        continue;
                    }
                };
                let cycle_state = match cycles.state_for_vehicle(vehicle_id).await {
                    Ok(state) => state,
                    Err(error) => {
                        log::error!(
                            "[{ACTOR}] vehicle {vehicle_id} cycle state: {}",
                            error.message
                        );
                        continue;
                    }
                };
                let away = snapshot.away || cycle_state.is_some_and(|cycle| cycle.cycle_is_open());
                let kind = transition_for_position(position, &settings, away, now);
                let mut recorded_kind = None;
                if let (Some(kind), Some(occurred_at)) = (kind, position.recorded_at) {
                    match presence
                        .record(
                            Some(tenant_id),
                            vehicle_id,
                            kind,
                            occurred_at.naive_utc(),
                            PresenceSource::Tracker,
                        )
                        .await
                    {
                        Ok((true, snapshot)) => {
                            recorded += 1;
                            recorded_kind = Some(kind);
                            if kind == PresenceEventKind::Arrival {
                                crate::endpoints::vehicle_presence_endpoint::open_triage_on_arrival(
                                    state,
                                    Some(tenant_id),
                                    vehicle_id,
                                    &snapshot,
                                    occurred_at.naive_utc(),
                                )
                                .await;
                            }
                        }
                        Ok((false, _)) => {}
                        Err(error) => {
                            log::error!("[{ACTOR}] vehicle {vehicle_id}: {}", error.message);
                        }
                    }
                }

                let Some(reported_at) = position.recorded_at else {
                    continue;
                };
                let cycle_sample = GarageCycleSample {
                    tenant_id: Some(tenant_id),
                    vehicle_id,
                    reported_at: reported_at.naive_utc(),
                    at_base: is_within_base(
                        position.latitude,
                        position.longitude,
                        settings.garage_base_latitude.unwrap_or_default(),
                        settings.garage_base_longitude.unwrap_or_default(),
                        settings.garage_base_radius_meters,
                    ),
                    transition: recorded_kind,
                    odometer_meters: position.odometer_meters,
                    ignition_on: position.ignition,
                };
                if let Err(error) = cycles.observe(cycle_sample, &settings).await {
                    log::error!("[{ACTOR}] vehicle {vehicle_id} cycle: {}", error.message);
                }
            }

            Ok::<usize, BusinessError>(recorded)
        })
        .await;

        match result {
            Ok(recorded) if recorded > 0 => {
                log::info!("[{ACTOR}] tenant {tenant_id}: recorded {recorded} presence transitions")
            }
            Ok(_) => {}
            Err(error) => log::error!("[{ACTOR}] tenant {tenant_id}: {}", error.message),
        }
    }
}

pub fn spawn_presence_poll(state: AppState) {
    let configured = [
        "TRACKING_API_BASE_URL",
        "TRACKING_API_EMAIL",
        "TRACKING_API_PASSWORD",
    ]
    .iter()
    .all(|key| std::env::var(key).is_ok_and(|value| !value.trim().is_empty()));
    if !configured {
        return log::info!("[{ACTOR}] disabled (tracking provider is not configured)");
    }

    let seconds: u64 = std::env::var("GARAGE_PRESENCE_POLL_INTERVAL_SECONDS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(60);
    if seconds == 0 {
        return log::info!("[{ACTOR}] disabled (GARAGE_PRESENCE_POLL_INTERVAL_SECONDS=0)");
    }

    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(seconds));
        loop {
            tick.tick().await;
            run_presence_poll_once(&state).await;
        }
    });
}
