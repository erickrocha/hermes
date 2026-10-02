use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::fuel_entry::{FuelEntry, FuelEntryEntityMapper};
use crate::domain::tenant_rule_setting::RuleSettings;
use crate::gateway::fuel_entry_gateway::FuelEntryGateway;
use crate::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use sea_orm::DbErr;

pub const VOLUME_MUST_BE_POSITIVE: &str = "A fuelling's volume must be positive";
pub const VALUE_MUST_NOT_BE_NEGATIVE: &str = "A fuelling's value cannot be negative";
pub const VEHICLE_NOT_FOUND: &str = "Vehicle not found";
pub const FUEL_ENTRY_NOT_FOUND: &str = "Fuel entry not found";
pub const NOTHING_TO_RESTORE: &str = "This vehicle has no deleted fuelling to restore";
pub const UNIFY_NEEDS_TWO_ENTRIES: &str = "Two different fuellings are needed to unify";
pub const UNIFY_DIFFERENT_VEHICLES: &str = "Only fuellings of the same vehicle can be unified";
pub const UNIFY_DIFFERENT_DAYS: &str = "Only fuellings of the same day can be unified";

pub struct FuelEntryUseCase {
    gateway: FuelEntryGateway,
    vehicles: VehicleGateway,
    rules: TenantRuleSettingGateway,
}

impl FuelEntryUseCase {
    pub fn new(gateway: FuelEntryGateway, vehicles: VehicleGateway, rules: TenantRuleSettingGateway) -> Self {
        Self { gateway, vehicles, rules }
    }

    pub async fn create(&self, entry: FuelEntry) -> Result<FuelEntry, BusinessError> {
        let entry = self.validated(entry).await?;
        let entity = self.gateway.persist(entry).await.map_err(database_error)?;
        Ok(FuelEntryEntityMapper::from_active_model(entity))
    }

    async fn validated(&self, entry: FuelEntry) -> Result<FuelEntry, BusinessError> {
        if entry.volume_liters <= 0.0 {
            return Err(BusinessError::new(VOLUME_MUST_BE_POSITIVE.to_string()));
        }
        if entry.value_cents < 0 {
            return Err(BusinessError::new(VALUE_MUST_NOT_BE_NEGATIVE.to_string()));
        }

        let vehicle = self
            .vehicles
            .find_by_id(entry.vehicle_id)
            .await
            .map_err(database_error)?;
        if !vehicle.is_some_and(|v| v.tenant_id == entry.tenant_id) {
            return Err(BusinessError::new(VEHICLE_NOT_FOUND.to_string()));
        }

        Ok(entry)
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<FuelEntry, BusinessError> {
        let entity = self.gateway.find_by_uuid(uuid).await.map_err(database_error)?;
        match entity {
            Some(model) => Ok(FuelEntryEntityMapper::from_model(model)),
            None => Err(BusinessError::new(FUEL_ENTRY_NOT_FOUND.to_string())),
        }
    }

    pub async fn find_page(&self, page: u64, page_size: u64) -> Result<(Vec<FuelEntry>, u64), BusinessError> {
        let (rows, total) = self.gateway.find_page(page, page_size).await.map_err(database_error)?;
        Ok((FuelEntryEntityMapper::from_models(rows), total))
    }
}

impl FuelEntryUseCase {
    /// `TRM-1553`: the fuelling list plus its litre and value totals.
    pub async fn report(
        &self,
        from: Option<chrono::NaiveDateTime>,
        to: Option<chrono::NaiveDateTime>,
        vehicle_id: Option<i64>,
        station: Option<String>,
    ) -> Result<(Vec<FuelEntry>, f64, i64), BusinessError> {
        let rows = self
            .gateway
            .find_report(from, to, vehicle_id, station)
            .await
            .map_err(database_error)?;
        let entries = FuelEntryEntityMapper::from_models(rows);
        let (liters, cents) = totals(&entries);
        Ok((entries, liters, cents))
    }
}

impl FuelEntryUseCase {
    /// `TRM-563…568`: a vehicle's own consumption average over its whole ledger.
    pub async fn average(&self, vehicle_id: i64) -> Result<ConsumptionAverage, BusinessError> {
        let rows = self
            .gateway
            .find_report(None, None, Some(vehicle_id), None)
            .await
            .map_err(database_error)?;
        let entries = FuelEntryEntityMapper::from_models(rows);
        let tenant_id = self.vehicles.find_by_id(vehicle_id).await.map_err(database_error)?.and_then(|v| v.tenant_id);
        let rules = self.rules.settings_for(tenant_id).await.map_err(database_error)?;
        Ok(consumption_average(&entries, chrono::Utc::now().naive_utc(), &rules))
    }
}

impl FuelEntryUseCase {
    /// `TRM-552`: a fuelling is only ever soft-deleted.
    pub async fn soft_delete(&self, uuid: String) -> Result<(), BusinessError> {
        let entry = self.find_by_uuid(uuid).await?;
        self.gateway
            .persist(FuelEntry { deleted_at: Some(chrono::Utc::now().naive_utc()), ..entry })
            .await
            .map_err(database_error)?;
        Ok(())
    }

    /// `TRM-553`: brings back the vehicle's most recently deleted fuelling with
    /// all its data intact.
    pub async fn restore_last(&self, vehicle_id: i64) -> Result<FuelEntry, BusinessError> {
        let row = self
            .gateway
            .find_last_deleted_by_vehicle(vehicle_id)
            .await
            .map_err(database_error)?
            .ok_or_else(|| BusinessError::new(NOTHING_TO_RESTORE.to_string()))?;
        let saved = self
            .gateway
            .persist(FuelEntry { deleted_at: None, ..FuelEntryEntityMapper::from_model(row) })
            .await
            .map_err(database_error)?;
        Ok(FuelEntryEntityMapper::from_active_model(saved))
    }

    /// `TRM-554`: two fuellings of the same vehicle on the same day become
    /// one -- volumes and values summed, an odometer taken from either where
    /// the keeper has none, a full tank if either was. The absorbed entry is
    /// soft-deleted and points at the keeper, which is annotated.
    pub async fn unify(&self, target_uuid: String, source_uuid: String) -> Result<FuelEntry, BusinessError> {
        if target_uuid == source_uuid {
            return Err(BusinessError::new(UNIFY_NEEDS_TWO_ENTRIES.to_string()));
        }
        let target = self.find_by_uuid(target_uuid).await?;
        let source = self.find_by_uuid(source_uuid).await?;
        if target.vehicle_id != source.vehicle_id {
            return Err(BusinessError::new(UNIFY_DIFFERENT_VEHICLES.to_string()));
        }
        if target.recorded_at.date() != source.recorded_at.date() {
            return Err(BusinessError::new(UNIFY_DIFFERENT_DAYS.to_string()));
        }
        let now = chrono::Utc::now().naive_utc();
        let note = format!("Unified with {} at {now}", source.uuid.clone().unwrap_or_default());
        let keeper = FuelEntry {
            volume_liters: target.volume_liters + source.volume_liters,
            value_cents: target.value_cents + source.value_cents,
            odometer_km: target.odometer_km.or(source.odometer_km),
            full_tank: target.full_tank || source.full_tank,
            unification_note: Some(match &target.unification_note {
                Some(previous) => format!("{previous} | {note}"),
                None => note,
            }),
            ..target
        };
        let keeper_id = keeper.id;
        let saved = self.gateway.persist(keeper).await.map_err(database_error)?;
        self.gateway
            .persist(FuelEntry { deleted_at: Some(now), unified_into_id: keeper_id, ..source })
            .await
            .map_err(database_error)?;
        Ok(FuelEntryEntityMapper::from_active_model(saved))
    }
}

/// `TRM-568`/`571`: `km_per_liter` is `None` until the average is mature, so
/// no borrowed or premature figure is ever presented as a measured one.
#[derive(Debug, Clone, PartialEq)]
pub struct ConsumptionAverage {
    pub km_per_liter: Option<f64>,
    pub segments: usize,
    pub mature: bool,
}

struct Segment {
    km: f64,
    liters: f64,
    closed_at: chrono::NaiveDateTime,
}

impl Segment {
    fn km_per_liter(&self) -> f64 {
        self.km / self.liters
    }
}

/// `TRM-563…567`: full-tank-to-full-tank segments (partial fuellings add their
/// litres to the open segment and never close one), implausible segments
/// discarded, then the recent window, median rejection, and total distance
/// over total litres -- not the mean of the segments' own averages.
pub fn consumption_average(entries: &[FuelEntry], now: chrono::NaiveDateTime, rules: &RuleSettings) -> ConsumptionAverage {
    let mut usable: Vec<&FuelEntry> = entries
        .iter()
        .filter(|e| e.odometer_km.is_some_and(|km| km > 0.0) && e.volume_liters > 0.0)
        .collect();
    usable.sort_by_key(|e| e.recorded_at);

    let mut segments: Vec<Segment> = Vec::new();
    let mut anchor: Option<f64> = None;
    let mut liters = 0.0;
    for entry in usable {
        let odometer = entry.odometer_km.unwrap_or_default();
        let Some(start) = anchor else {
            if entry.full_tank {
                anchor = Some(odometer);
            }
            continue;
        };
        liters += entry.volume_liters;
        if !entry.full_tank {
            continue;
        }
        let km = odometer - start;
        let kml = km / liters;
        if km > 0.0 && km <= rules.avg_max_segment_km && kml >= rules.avg_min_km_per_liter && kml <= rules.avg_max_km_per_liter {
            segments.push(Segment { km, liters, closed_at: entry.recorded_at });
        }
        anchor = Some(odometer);
        liters = 0.0;
    }

    let cutoff = now - chrono::Duration::days(rules.avg_recent_window_days as i64);
    let recent: Vec<&Segment> = segments.iter().filter(|s| s.closed_at >= cutoff).collect();
    let mut chosen: Vec<&Segment> = if recent.is_empty() {
        segments.iter().rev().take(rules.avg_fallback_segments as usize).rev().collect()
    } else {
        recent
    };

    if chosen.len() >= rules.avg_median_min_segments as usize {
        let mut sorted: Vec<f64> = chosen.iter().map(|s| s.km_per_liter()).collect();
        sorted.sort_by(|a, b| a.total_cmp(b));
        let mid = sorted.len() / 2;
        let median = if sorted.len() % 2 == 1 { sorted[mid] } else { (sorted[mid - 1] + sorted[mid]) / 2.0 };
        let within: Vec<&Segment> = chosen
            .iter()
            .copied()
            .filter(|s| {
                let kml = s.km_per_liter();
                kml >= median * (1.0 - rules.avg_median_tolerance) && kml <= median * (1.0 + rules.avg_median_tolerance)
            })
            .collect();
        if within.len() >= rules.avg_median_min_survivors as usize {
            chosen = within;
        }
    }

    let (km, l) = chosen.iter().fold((0.0, 0.0), |(km, l), s| (km + s.km, l + s.liters));
    let mature = chosen.len() >= rules.avg_min_segments_for_own_average as usize && l > 0.0;
    ConsumptionAverage { km_per_liter: mature.then(|| km / l), segments: chosen.len(), mature }
}

fn totals(entries: &[FuelEntry]) -> (f64, i64) {
    entries
        .iter()
        .fold((0.0, 0), |(l, c), e| (l + e.volume_liters, c + e.value_cents))
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[FuelEntryUseCase] {}", msg);
    BusinessError::new(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::enums::FuelEntryOrigin;

    fn entry(liters: f64, cents: i64) -> FuelEntry {
        FuelEntry {
            id: None,
            uuid: None,
            tenant_id: Some(1),
            vehicle_id: 1,
            recorded_at: chrono::NaiveDateTime::default(),
            volume_liters: liters,
            value_cents: cents,
            odometer_km: None,
            station: None,
            full_tank: false,
            origin: FuelEntryOrigin::Manual,
            provider_transaction_id: None,
            reported_by_user_id: None,
            odometer_override_note: None,
            provider_confirmed_at: None,
            deleted_at: None,
            unified_into_id: None,
            unification_note: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
    }

    #[test]
    fn totals_sum_litres_and_value() {
        assert_eq!(totals(&[]), (0.0, 0));
        assert_eq!(totals(&[entry(40.5, 25000), entry(10.0, 6000)]), (50.5, 31000));
    }

    fn fill(day: u32, km: f64, liters: f64, full: bool) -> FuelEntry {
        FuelEntry {
            odometer_km: Some(km),
            full_tank: full,
            recorded_at: chrono::NaiveDate::from_ymd_opt(2026, 9, day).unwrap().and_hms_opt(8, 0, 0).unwrap(),
            ..entry(liters, 0)
        }
    }

    fn now() -> chrono::NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 9, 30).unwrap().and_hms_opt(0, 0, 0).unwrap()
    }

    #[test]
    fn partial_fuellings_add_to_the_segment_and_never_close_one() {
        // full@1000, partial 40 l, full 60 l @1300 -> 300 km / 100 l = 3 km/l.
        let ledger = [fill(1, 1000.0, 90.0, true), fill(2, 1150.0, 40.0, false), fill(3, 1300.0, 60.0, true)];
        let a = consumption_average(&ledger, now(), &RuleSettings::default());
        assert_eq!((a.segments, a.mature, a.km_per_liter), (1, false, None));
    }

    #[test]
    fn the_average_is_total_distance_over_total_litres_once_three_segments_exist() {
        // Segments of 3.0, 5.0 and 2.0 km/l.
        let ledger = [
            fill(1, 1000.0, 1.0, true),
            fill(2, 1300.0, 100.0, true), // 3.0
            fill(3, 1500.0, 40.0, true),  // 5.0
            fill(4, 1600.0, 50.0, true),  // 2.0
        ];
        let a = consumption_average(&ledger, now(), &RuleSettings::default());
        assert!(a.mature && a.segments == 3);
        // distance/litres = 600/190, whereas the mean of the three averages would be 3.33.
        assert!((a.km_per_liter.unwrap() - 600.0 / 190.0).abs() < 1e-9);
    }

    #[test]
    fn implausible_segments_are_discarded() {
        // 5,000 km in one segment (> 4,000) and a non-advancing odometer are dropped.
        let ledger = [fill(1, 1000.0, 1.0, true), fill(2, 6000.0, 1000.0, true), fill(3, 6000.0, 10.0, true)];
        assert_eq!(consumption_average(&ledger, now(), &RuleSettings::default()).segments, 0);
    }

    #[test]
    fn a_segment_far_from_the_median_is_rejected_when_enough_survive() {
        // Four segments at 3 km/l and one wrongly "full" at 6 km/l: the 6 is > 40 % over the median.
        let mut ledger = vec![fill(1, 1000.0, 1.0, true)];
        for (i, km) in [1300.0, 1600.0, 1900.0, 2200.0].iter().enumerate() {
            ledger.push(fill(2 + i as u32, *km, 100.0, true));
        }
        ledger.push(fill(7, 2500.0, 50.0, true)); // 300 km / 50 l = 6.0
        let a = consumption_average(&ledger, now(), &RuleSettings::default());
        assert_eq!(a.segments, 4);
        assert!((a.km_per_liter.unwrap() - 3.0).abs() < 1e-9);
    }

    #[test]
    fn without_a_recent_segment_the_last_eight_are_used() {
        let old = |day: u32, km: f64| {
            let mut e = fill(day, km, 100.0, true);
            e.recorded_at -= chrono::Duration::days(120);
            e
        };
        let mut first = fill(1, 1000.0, 1.0, true);
        first.recorded_at -= chrono::Duration::days(120);
        let ledger = [first, old(2, 1300.0), old(3, 1600.0), old(4, 1900.0)];
        let a = consumption_average(&ledger, now(), &RuleSettings::default());
        assert!(a.mature && (a.km_per_liter.unwrap() - 3.0).abs() < 1e-9);
    }

    #[test]
    fn a_tenant_that_trusts_a_single_segment_gets_a_mature_average_from_one() {
        let ledger = [fill(1, 1000.0, 1.0, true), fill(2, 1300.0, 100.0, true)];
        assert!(!consumption_average(&ledger, now(), &RuleSettings::default()).mature);
        let eager = RuleSettings { avg_min_segments_for_own_average: 1, ..RuleSettings::default() };
        let a = consumption_average(&ledger, now(), &eager);
        assert!(a.mature && (a.km_per_liter.unwrap() - 3.0).abs() < 1e-9);
        // A tighter plausible band drops the same 3 km/l segment altogether.
        let tight = RuleSettings { avg_max_km_per_liter: 2.0, ..eager };
        assert_eq!(consumption_average(&ledger, now(), &tight).segments, 0);
    }
}
