use crate::AppState;
use business::commons::gateway::Gateway;
use business::domain::business_error::BusinessError;
use business::domain::enums::PresenceSource;
use business::gateway::tenant_gateway::TenantGateway;
use business::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway;
use business::gateway::tracking_provider::{TrackingProvider, configured_provider};
use business::gateway::vehicle_gateway::{VehicleGateway, VehicleStore};
use business::gateway::vehicle_presence_event_gateway::VehiclePresenceEventGateway;
use business::use_cases::garage_geofence::transition_for_position;
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
                let snapshot = match presence.snapshot(vehicle_id).await {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        log::error!("[{ACTOR}] vehicle {vehicle_id}: {}", error.message);
                        continue;
                    }
                };
                let Some(kind) = transition_for_position(position, &settings, snapshot.away, now)
                else {
                    continue;
                };
                let Some(occurred_at) = position.recorded_at else {
                    continue;
                };

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
                        if kind == business::domain::enums::PresenceEventKind::Arrival {
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
