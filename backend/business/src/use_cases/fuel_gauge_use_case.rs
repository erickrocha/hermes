use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::fuel_entry::{FuelEntry, FuelEntryEntityMapper};
use crate::domain::fuel_gauge_setting::{FuelGaugeSetting, FuelGaugeSettingEntityMapper};
use crate::gateway::fuel_gauge_setting_gateway::FuelGaugeSettingGateway;
use crate::gateway::fuel_entry_gateway::FuelEntryGateway;
use crate::gateway::km_evolution_gateway::KmEvolutionGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use crate::domain::tenant_rule_setting::RuleSettings;
use crate::gateway::tenant_rule_setting_gateway::TenantRuleSettingGateway;
use crate::use_cases::fuel_entry_use_case::consumption_average;
use chrono::NaiveDateTime;
use sea_orm::DbErr;

/// A tenant's gauge thresholds (`PD-016`'s `[TC]` values). Without a row of its
/// own a tenant gets the legacy values below; they are defaults, not rules.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaugeSettings {
    /// `TRM-596`: a reading implying more than this multiple of the tank's
    /// capacity has been consumed is the datum's fault, not the vehicle's.
    pub suspect_margin_ratio: f64,
    /// `TRM-589`: a fuelling is only judged a top-off when the litres the
    /// distance implies exceed this.
    pub set_aside_min_expected_liters: f64,
    /// `TRM-589`: ... and the fuelling is below this share of those litres.
    pub set_aside_ratio: f64,
    /// `TRM-592`: a fuelling of at least this share of the capacity is never set aside.
    pub large_fuelling_ratio: f64,
}

impl Default for GaugeSettings {
    fn default() -> Self {
        Self { suspect_margin_ratio: 1.05, set_aside_min_expected_liters: 8.0, set_aside_ratio: 0.5, large_fuelling_ratio: 0.5 }
    }
}

impl From<&FuelGaugeSetting> for GaugeSettings {
    fn from(s: &FuelGaugeSetting) -> Self {
        Self {
            suspect_margin_ratio: s.suspect_margin_ratio,
            set_aside_min_expected_liters: s.set_aside_min_expected_liters,
            set_aside_ratio: s.set_aside_ratio,
            large_fuelling_ratio: s.large_fuelling_ratio,
        }
    }
}

/// `TRM-597`: why there is no usable reading -- never one undifferentiated
/// absence.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GaugeUnavailable {
    /// `TRM-581`: no tank capacity is registered.
    NoTankRegistered,
    /// No full-tank fuelling exists to start the count from.
    NoReferenceFuelling,
    /// The distance since the anchor, or the vehicle's own consumption, cannot
    /// be known yet.
    AwaitingCalibration,
    /// `TRM-596`: the implied consumption exceeds the tank.
    Suspect,
}

/// `TRM-569`/`570`/`571`: where the consumption behind a reading comes from.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConsumptionSource {
    /// The vehicle's own mature average.
    Own,
    /// The registered per-vehicle reference (`TRM-569`).
    Reference,
    /// The median of comparable-tank peers' mature averages (`TRM-570`).
    PeerMedian,
}

/// The consumption a vehicle's reading rests on; `None` when nothing can be offered.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectiveConsumption {
    pub km_per_liter: Option<f64>,
    pub source: Option<ConsumptionSource>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TankGauge {
    Reading {
        percent: f64,
        remaining_liters: f64,
        range_km: f64,
        /// `TRM-580`: the consumption the reading rests on, km per litre.
        consumption_km_per_liter: f64,
        anchored_at: NaiveDateTime,
        /// `TRM-571`: true when the consumption is a reference or peer estimate, not the vehicle's own.
        estimated: bool,
        /// `TRM-589`: fuellings flagged full that were judged top-offs and credited as partials.
        set_aside_at: Vec<NaiveDateTime>,
    },
    Unavailable(GaugeUnavailable),
}

/// `EPIC-FU-06-S02`/`S03` (`HRMS-952`/`953`): a vehicle's estimated tank level.
pub struct FuelGaugeUseCase {
    fuel: FuelEntryGateway,
    vehicles: VehicleGateway,
    km: KmEvolutionGateway,
    settings: FuelGaugeSettingGateway,
    rules: TenantRuleSettingGateway,
}

impl FuelGaugeUseCase {
    pub fn new(
        fuel: FuelEntryGateway,
        vehicles: VehicleGateway,
        km: KmEvolutionGateway,
        settings: FuelGaugeSettingGateway,
        rules: TenantRuleSettingGateway,
    ) -> Self {
        Self { fuel, vehicles, km, settings, rules }
    }

    pub async fn gauge(&self, vehicle_id: i64) -> Result<TankGauge, BusinessError> {
        let vehicle = self.vehicles.find_by_id(vehicle_id).await.map_err(database_error)?;
        let (capacity, reference, tenant_id) = vehicle
            .map(|v| (v.tank_capacity_liters, v.reference_km_per_liter, v.tenant_id))
            .unwrap_or_default();
        let settings = self
            .settings
            .find_current(tenant_id)
            .await
            .map_err(database_error)?
            .map(|m| GaugeSettings::from(&FuelGaugeSettingEntityMapper::from_model(m)))
            .unwrap_or_default();
        let rows = self.fuel.find_report(None, None, Some(vehicle_id), None).await.map_err(database_error)?;
        let mut entries = FuelEntryEntityMapper::from_models(rows);
        // A full tank with no odometer of its own takes the official record's
        // reading at its time (`TRM-583`: the distance never comes from summing).
        for entry in entries.iter_mut().filter(|e| e.full_tank && e.odometer_km.is_none()) {
            entry.odometer_km = self
                .km
                .find_before(vehicle_id, entry.recorded_at)
                .await
                .map_err(database_error)?
                .map(|r| r.km);
        }
        let current_km = self.km.find_latest_by_vehicle(vehicle_id).await.map_err(database_error)?.map(|r| r.km);
        let rules = self.rules.settings_for(tenant_id).await.map_err(database_error)?;
        let now = chrono::Utc::now().naive_utc();
        // Only an immature own average needs a borrowed figure.
        let fallback = if consumption_average(&entries, now, &rules).mature {
            None
        } else {
            self.fallback_consumption(vehicle_id, capacity, reference, &rules).await?.map(|(kml, _)| kml)
        };
        Ok(gauge(capacity, &entries, current_km, &settings, &rules, fallback, now))
    }

    /// `TRM-569`/`570`/`571`: the consumption to use, and where it came from --
    /// the vehicle's own mature average, else its registered reference, else the
    /// median of comparable peers. Anything but the first is an estimate.
    pub async fn effective_consumption(&self, vehicle_id: i64) -> Result<EffectiveConsumption, BusinessError> {
        let vehicle = self.vehicles.find_by_id(vehicle_id).await.map_err(database_error)?;
        let (capacity, reference, tenant_id) = vehicle
            .map(|v| (v.tank_capacity_liters, v.reference_km_per_liter, v.tenant_id))
            .unwrap_or_default();
        let rules = self.rules.settings_for(tenant_id).await.map_err(database_error)?;
        let rows = self.fuel.find_report(None, None, Some(vehicle_id), None).await.map_err(database_error)?;
        let entries = FuelEntryEntityMapper::from_models(rows);
        let own = consumption_average(&entries, chrono::Utc::now().naive_utc(), &rules);
        if own.mature {
            return Ok(EffectiveConsumption { km_per_liter: own.km_per_liter, source: Some(ConsumptionSource::Own) });
        }
        Ok(match self.fallback_consumption(vehicle_id, capacity, reference, &rules).await? {
            Some((kml, source)) => EffectiveConsumption { km_per_liter: Some(kml), source: Some(source) },
            None => EffectiveConsumption { km_per_liter: None, source: None },
        })
    }

    async fn fallback_consumption(
        &self,
        vehicle_id: i64,
        capacity: Option<f64>,
        reference: Option<f64>,
        rules: &RuleSettings,
    ) -> Result<Option<(f64, ConsumptionSource)>, BusinessError> {
        if let Some(reference) = reference.filter(|r| *r > 0.0) {
            return Ok(Some((reference, ConsumptionSource::Reference)));
        }
        let Some(capacity) = capacity.filter(|c| *c > 0.0) else { return Ok(None) };
        let now = chrono::Utc::now().naive_utc();
        let mut peers = Vec::new();
        for peer in self.vehicles.find_all().await.map_err(database_error)? {
            let comparable = peer.id != vehicle_id
                && peer.status != "Inactive"
                && peer.tank_capacity_liters.is_some_and(|c| c > 0.0 && (c - capacity).abs() <= capacity * rules.avg_peer_capacity_tolerance);
            if !comparable {
                continue;
            }
            let rows = self.fuel.find_report(None, None, Some(peer.id), None).await.map_err(database_error)?;
            let average = consumption_average(&FuelEntryEntityMapper::from_models(rows), now, rules);
            if let Some(kml) = average.km_per_liter {
                peers.push(kml);
            }
        }
        Ok(peer_median(peers, rules).map(|kml| (kml, ConsumptionSource::PeerMedian)))
    }
}

/// `TRM-570`: the median of the comparable peers' mature averages, needing at
/// least `avg_peer_min_count` of them -- fewer is no basis for a figure.
pub fn peer_median(mut peers: Vec<f64>, rules: &RuleSettings) -> Option<f64> {
    if peers.is_empty() || peers.len() < rules.avg_peer_min_count as usize {
        return None;
    }
    peers.sort_by(|a, b| a.total_cmp(b));
    let mid = peers.len() / 2;
    Some(if peers.len() % 2 == 1 { peers[mid] } else { (peers[mid - 1] + peers[mid]) / 2.0 })
}

/// `TRM-582`/`589…592`: the anchor is the latest full-tank fuelling **by its
/// own datetime** -- a transaction posted late but dated earlier never moves
/// it -- except that a "full" far below what the distance since the previous
/// anchor implies is a nozzle top-off (`TRM-589`), credited as a partial
/// instead. The judgement is not stored: it is remade on every read against
/// the current consumption (`TRM-591`), and a fuelling of at least
/// `large_fuelling_ratio` of the capacity is never set aside (`TRM-592`).
type Resolved<'a> = (&'a FuelEntry, Vec<&'a FuelEntry>, Vec<&'a FuelEntry>);

fn resolve_anchor<'a>(
    entries: &'a [FuelEntry],
    capacity: f64,
    consumption: f64,
    settings: &GaugeSettings,
) -> Option<Resolved<'a>> {
    let mut ordered: Vec<&FuelEntry> = entries.iter().collect();
    ordered.sort_by_key(|e| e.recorded_at);
    let mut anchor: Option<&FuelEntry> = None;
    let mut credited: Vec<&FuelEntry> = Vec::new();
    let mut set_aside: Vec<&FuelEntry> = Vec::new();
    for entry in ordered {
        if !entry.full_tank {
            // A partial before any anchor has nothing to be credited against.
            if anchor.is_some() {
                credited.push(entry);
            }
            continue;
        }
        let Some(previous) = anchor else {
            anchor = Some(entry);
            continue;
        };
        // `TRM-590`: where the implied litres are small the two agree, so a
        // small full tank anchors normally; a full tank with no odometer to
        // judge it by is accepted.
        let expected_liters = match (previous.odometer_km, entry.odometer_km) {
            (Some(from), Some(to)) if to > from => (to - from) / consumption,
            _ => 0.0,
        };
        let looks_like_a_top_off = expected_liters > settings.set_aside_min_expected_liters
            && entry.volume_liters < expected_liters * settings.set_aside_ratio;
        let large = entry.volume_liters >= capacity * settings.large_fuelling_ratio;
        if looks_like_a_top_off && !large {
            set_aside.push(entry);
            credited.push(entry);
        } else {
            anchor = Some(entry);
            credited.clear();
            set_aside.clear();
        }
    }
    anchor.map(|a| (a, credited, set_aside))
}

/// `TRM-580`/`581`/`588`/`589…592`/`596`/`597`: capacity less the distance run
/// since the anchor over the effective consumption, with the litres of any
/// partial (or set-aside) fuelling since credited back.
pub fn gauge(
    capacity: Option<f64>,
    entries: &[FuelEntry],
    current_km: Option<f64>,
    settings: &GaugeSettings,
    rules: &RuleSettings,
    fallback_km_per_liter: Option<f64>,
    now: NaiveDateTime,
) -> TankGauge {
    let Some(capacity) = capacity.filter(|c| *c > 0.0) else {
        return TankGauge::Unavailable(GaugeUnavailable::NoTankRegistered);
    };
    if !entries.iter().any(|e| e.full_tank) {
        return TankGauge::Unavailable(GaugeUnavailable::NoReferenceFuelling);
    }
    // `TRM-568`/`569`/`570`: the vehicle's own average once mature, else the
    // reference or peer figure the caller resolved -- and then say so (`TRM-571`).
    let own = consumption_average(entries, now, rules).km_per_liter;
    let (Some(consumption), estimated) = (own.or(fallback_km_per_liter), own.is_none()) else {
        return TankGauge::Unavailable(GaugeUnavailable::AwaitingCalibration);
    };
    let Some((anchor, credited, set_aside)) = resolve_anchor(entries, capacity, consumption, settings) else {
        return TankGauge::Unavailable(GaugeUnavailable::NoReferenceFuelling);
    };
    let (Some(anchor_km), Some(current_km)) = (anchor.odometer_km, current_km) else {
        return TankGauge::Unavailable(GaugeUnavailable::AwaitingCalibration);
    };
    // Forwards only: an odometer behind the anchor cannot be reasoned from.
    if current_km < anchor_km {
        return TankGauge::Unavailable(GaugeUnavailable::AwaitingCalibration);
    }

    // `TRM-588`: a partial fuelling after the anchor gives back the distance
    // its litres are worth.
    let credited_km: f64 = credited.iter().map(|e| e.volume_liters * consumption).sum();
    let used_liters = ((current_km - anchor_km - credited_km) / consumption).max(0.0);
    if used_liters > capacity * settings.suspect_margin_ratio {
        return TankGauge::Unavailable(GaugeUnavailable::Suspect);
    }
    let remaining_liters = (capacity - used_liters).max(0.0);
    TankGauge::Reading {
        percent: remaining_liters / capacity * 100.0,
        remaining_liters,
        range_km: remaining_liters * consumption,
        consumption_km_per_liter: consumption,
        anchored_at: anchor.recorded_at,
        estimated,
        set_aside_at: set_aside.iter().map(|e| e.recorded_at).collect(),
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[FuelGaugeUseCase] {}", msg);
    BusinessError::new(msg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::enums::FuelEntryOrigin;
    use chrono::NaiveDate;

    fn at(day: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, day).unwrap().and_hms_opt(8, 0, 0).unwrap()
    }

    fn fill(day: u32, km: Option<f64>, liters: f64, full: bool) -> FuelEntry {
        FuelEntry {
            id: None,
            uuid: None,
            tenant_id: Some(1),
            vehicle_id: 1,
            recorded_at: at(day),
            volume_liters: liters,
            value_cents: 0,
            odometer_km: km,
            station: None,
            full_tank: full,
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

    /// Four full tanks of 100 l each 300 km apart: a mature 3.0 km/l, then the
    /// last one at 1,900 km is the anchor.
    fn history() -> Vec<FuelEntry> {
        vec![
            fill(1, Some(1_000.0), 1.0, true),
            fill(2, Some(1_300.0), 100.0, true),
            fill(3, Some(1_600.0), 100.0, true),
            fill(4, Some(1_900.0), 100.0, true),
        ]
    }

    fn now() -> NaiveDateTime {
        at(10)
    }

    #[test]
    fn the_level_is_capacity_less_the_distance_run_over_the_consumption() {
        // 150 km since the anchor at 3 km/l = 50 l used of 300 l.
        let g = gauge(Some(300.0), &history(), Some(2_050.0), &GaugeSettings::default(), &RuleSettings::default(), None, now());
        let TankGauge::Reading { percent, remaining_liters, range_km, consumption_km_per_liter, .. } = g else {
            panic!("expected a reading, got {g:?}");
        };
        assert!((consumption_km_per_liter - 3.0).abs() < 1e-9);
        assert!((remaining_liters - 250.0).abs() < 1e-9);
        assert!((percent - 250.0 / 300.0 * 100.0).abs() < 1e-9);
        assert!((range_km - 750.0).abs() < 1e-9);
    }

    #[test]
    fn a_partial_fuelling_after_the_anchor_is_credited_back() {
        let mut ledger = history();
        ledger.push(fill(5, Some(2_050.0), 30.0, false)); // 30 l * 3 km/l = 90 km credited
        let TankGauge::Reading { remaining_liters, .. } = gauge(Some(300.0), &ledger, Some(2_050.0), &GaugeSettings::default(), &RuleSettings::default(), None, now()) else {
            panic!("expected a reading");
        };
        // (150 - 90) / 3 = 20 l used.
        assert!((remaining_liters - 280.0).abs() < 1e-9);
    }

    #[test]
    fn the_anchor_is_the_latest_full_tank_by_its_own_datetime_not_by_insertion_order() {
        let mut ledger = history();
        // Posted last but dated before the current anchor: history only, never the anchor.
        ledger.push(fill(2, Some(1_300.0), 100.0, true));
        let TankGauge::Reading { anchored_at, .. } = gauge(Some(300.0), &ledger, Some(2_050.0), &GaugeSettings::default(), &RuleSettings::default(), None, now()) else {
            panic!("expected a reading");
        };
        assert_eq!(anchored_at, at(4));
    }

    #[test]
    fn each_absence_of_a_reading_says_why() {
        let h = history();
        assert_eq!(gauge(None, &h, Some(2_000.0), &GaugeSettings::default(), &RuleSettings::default(), None, now()), TankGauge::Unavailable(GaugeUnavailable::NoTankRegistered));
        assert_eq!(gauge(Some(300.0), &[], Some(2_000.0), &GaugeSettings::default(), &RuleSettings::default(), None, now()), TankGauge::Unavailable(GaugeUnavailable::NoReferenceFuelling));
        // Too short a history for the vehicle's own consumption.
        assert_eq!(gauge(Some(300.0), &h[..2], Some(2_000.0), &GaugeSettings::default(), &RuleSettings::default(), None, now()), TankGauge::Unavailable(GaugeUnavailable::AwaitingCalibration));
        // No current odometer, and an odometer behind the anchor.
        assert_eq!(gauge(Some(300.0), &h, None, &GaugeSettings::default(), &RuleSettings::default(), None, now()), TankGauge::Unavailable(GaugeUnavailable::AwaitingCalibration));
        assert_eq!(gauge(Some(300.0), &h, Some(1_800.0), &GaugeSettings::default(), &RuleSettings::default(), None, now()), TankGauge::Unavailable(GaugeUnavailable::AwaitingCalibration));
    }

    #[test]
    fn a_reading_that_implies_more_than_the_tank_holds_is_suspect_not_empty() {
        // 1,000 km at 3 km/l = 333 l against a 300 l tank: > 105 %.
        assert_eq!(gauge(Some(300.0), &history(), Some(2_900.0), &GaugeSettings::default(), &RuleSettings::default(), None, now()), TankGauge::Unavailable(GaugeUnavailable::Suspect));
        // 940 km = 313 l, inside the 105 % margin: an empty tank, not suspect.
        let TankGauge::Reading { remaining_liters, .. } = gauge(Some(300.0), &history(), Some(2_840.0), &GaugeSettings::default(), &RuleSettings::default(), None, now()) else {
            panic!("expected a reading");
        };
        assert_eq!(remaining_liters, 0.0);
    }

    #[test]
    fn an_anchor_with_no_odometer_cannot_anchor_a_reading() {
        // The consumption needs three measured segments; the anchor itself has no odometer.
        let mut ledger = history();
        ledger.push(fill(5, None, 100.0, true));
        assert_eq!(
            gauge(Some(300.0), &ledger, Some(2_050.0), &GaugeSettings::default(), &RuleSettings::default(), None, now()),
            TankGauge::Unavailable(GaugeUnavailable::AwaitingCalibration)
        );
    }

    #[test]
    fn a_tiny_full_tank_after_running_is_set_aside_and_credited_not_anchored() {
        // 300 km since the anchor at 1,900 km = 100 l expected; 1.15 l "full" is a top-off.
        let mut ledger = history();
        ledger.push(fill(5, Some(2_200.0), 1.15, true));
        let TankGauge::Reading { anchored_at, remaining_liters, set_aside_at, .. } =
            gauge(Some(300.0), &ledger, Some(2_200.0), &GaugeSettings::default(), &RuleSettings::default(), None, now())
        else {
            panic!("expected a reading");
        };
        assert_eq!(anchored_at, at(4), "the anchor stays on the last real full tank");
        assert_eq!(set_aside_at, vec![at(5)]);
        // 300 km run, 1.15 l * 3 = 3.45 km credited.
        assert!((remaining_liters - (300.0 - (300.0 - 3.45) / 3.0)).abs() < 1e-9);
    }

    #[test]
    fn a_small_full_tank_where_little_was_expected_still_anchors() {
        // 15 km since the anchor = 5 l expected (under the 8 l floor): 1.3 l agrees.
        let mut ledger = history();
        ledger.push(fill(5, Some(1_915.0), 1.3, true));
        let TankGauge::Reading { anchored_at, set_aside_at, .. } =
            gauge(Some(300.0), &ledger, Some(1_915.0), &GaugeSettings::default(), &RuleSettings::default(), None, now())
        else {
            panic!("expected a reading");
        };
        assert_eq!(anchored_at, at(5));
        assert!(set_aside_at.is_empty());
    }

    #[test]
    fn a_large_fuelling_is_never_set_aside() {
        // 2,400 km since the anchor = 800 l expected; 160 l is under half of that
        // but over half the 300 l tank, so it is the anchor.
        let mut ledger = history();
        ledger.push(fill(5, Some(4_300.0), 160.0, true));
        let TankGauge::Reading { anchored_at, .. } =
            gauge(Some(300.0), &ledger, Some(4_300.0), &GaugeSettings::default(), &RuleSettings::default(), None, now())
        else {
            panic!("expected a reading");
        };
        assert_eq!(anchored_at, at(5));
    }

    #[test]
    fn a_tenant_can_move_the_thresholds() {
        let mut ledger = history();
        ledger.push(fill(5, Some(2_200.0), 1.15, true));
        // A tenant whose set-aside floor is above the 100 l expected never sets this one aside.
        let lenient = GaugeSettings { set_aside_min_expected_liters: 150.0, ..GaugeSettings::default() };
        let TankGauge::Reading { anchored_at, .. } = gauge(Some(300.0), &ledger, Some(2_200.0), &lenient, &RuleSettings::default(), None, now()) else {
            panic!("expected a reading");
        };
        assert_eq!(anchored_at, at(5));
        // A stricter suspect margin turns an otherwise valid reading suspect.
        let strict = GaugeSettings { suspect_margin_ratio: 0.5, ..GaugeSettings::default() };
        assert_eq!(
            gauge(Some(300.0), &history(), Some(2_900.0), &strict, &RuleSettings::default(), None, now()),
            TankGauge::Unavailable(GaugeUnavailable::Suspect)
        );
    }

    #[test]
    fn a_young_vehicle_borrows_a_reference_or_peer_consumption_and_says_so() {
        // Only one measured segment: the vehicle's own average is immature.
        let ledger = [fill(1, Some(1_000.0), 1.0, true), fill(2, Some(1_300.0), 100.0, true)];
        let d = GaugeSettings::default();
        let r = RuleSettings::default();
        assert_eq!(gauge(Some(300.0), &ledger, Some(1_450.0), &d, &r, None, now()), TankGauge::Unavailable(GaugeUnavailable::AwaitingCalibration));
        let TankGauge::Reading { estimated, remaining_liters, consumption_km_per_liter, .. } =
            gauge(Some(300.0), &ledger, Some(1_450.0), &d, &r, Some(3.0), now())
        else {
            panic!("expected an estimated reading");
        };
        assert!(estimated);
        assert_eq!(consumption_km_per_liter, 3.0);
        assert!((remaining_liters - 250.0).abs() < 1e-9);
        // A mature own average is never overridden by a fallback, and is not flagged.
        let TankGauge::Reading { estimated, consumption_km_per_liter, .. } =
            gauge(Some(300.0), &history(), Some(2_050.0), &d, &r, Some(9.0), now())
        else {
            panic!("expected a reading");
        };
        assert!(!estimated);
        assert!((consumption_km_per_liter - 3.0).abs() < 1e-9);
    }

    #[test]
    fn the_peer_median_needs_enough_peers() {
        let r = RuleSettings::default(); // at least 2 peers
        assert_eq!(peer_median(vec![], &r), None);
        assert_eq!(peer_median(vec![3.0], &r), None);
        assert_eq!(peer_median(vec![3.0, 5.0], &r), Some(4.0));
        assert_eq!(peer_median(vec![9.0, 3.0, 4.0], &r), Some(4.0));
        let lone = RuleSettings { avg_peer_min_count: 1, ..r };
        assert_eq!(peer_median(vec![3.0], &lone), Some(3.0));
    }
}
