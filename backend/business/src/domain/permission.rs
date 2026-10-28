//! `C-032`/`C-035` (`D-27`, `HRMS-850`): the platform's catalogue of permissions -- one per resource and
//! action, grouped into modules only for navigation. Generated from
//! `02-system_requirements/hermes/access-model_permission_catalogue.md`; edit that, not this.
//!
//! Every permission has a [`Scope`] that says which profiles may hold it:
//! - [`Scope::Tenant`]: held inside a tenant -- what a tenant profile (system or tenant-made) grants.
//! - [`Scope::Platform`]: the platform's own operations (tenants, plans, reference data, billing
//!   administration). Only a [`ProfileScope::Platform`] profile holds them.
//! - [`Scope::Baseline`]: every authenticated user, always; never granted, never revoked.
//!
//! The `tenant_user` / `driver` / `mechanic` flags describe the three built-in tenant profiles below the
//! owner; the owner's and the platform administrator's system profiles hold every permission of their
//! scope. A grant is a permission code, or `code@own` for a permission with `own: true` -- held over the
//! caller's own records only (`AC-04`). See [`Grants`].

use crate::domain::enums::ProfileScope;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Tenant,
    Platform,
    Baseline,
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Scope::Tenant => "Tenant",
            Scope::Platform => "Platform",
            Scope::Baseline => "Baseline",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permission {
    pub code: &'static str,
    pub module: &'static str,
    pub scope: Scope,
    pub tenant_user: bool,
    pub driver: bool,
    pub mechanic: bool,
    /// May be granted over the user's own records only (`AC-04`) -- as `code@own`.
    pub own: bool,
}

/// `driver.act`: the one permission that is a capability of the person rather than of a screen -- the
/// holder may be named as the driver of a vehicle, a trip, a checklist run or a fuelling, and the driver
/// app lets them in.
pub const DRIVER_ACT: &str = "driver.act";

/// `garage.device.manage` (`HRMS-418`, `D-16`): pairing and revoking the wall-display credentials of the
/// tenant's garage monitors -- the owner's by default.
pub const GARAGE_DEVICE_MANAGE: &str = "garage.device.manage";

pub const CATALOGUE: &[Permission] = &[
    Permission { code: "profile.assign", module: "Access", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "profile.create", module: "Access", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "profile.read", module: "Access", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "profile.update", module: "Access", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "user.change-own-password", module: "Access", scope: Scope::Baseline, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "user.create", module: "Access", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "user.invite", module: "Access", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "user.read-own", module: "Access", scope: Scope::Baseline, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "user.update", module: "Access", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "billing.configure", module: "Billing", scope: Scope::Platform, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "billing.confirm-payment", module: "Billing", scope: Scope::Platform, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "billing.read", module: "Billing", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "billing.suspend", module: "Billing", scope: Scope::Platform, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "checklist-run.create", module: "Checklists", scope: Scope::Tenant, tenant_user: false, driver: true, mechanic: false, own: true },
    Permission { code: "checklist-run.read", module: "Checklists", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: true },
    Permission { code: "checklist-template.create", module: "Checklists", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "checklist-template.read", module: "Checklists", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "driver.act", module: "Fleet", scope: Scope::Tenant, tenant_user: false, driver: true, mechanic: false, own: false },
    Permission { code: "vehicle-presence.publish", module: "Fleet", scope: Scope::Platform, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "vehicle-presence.read", module: "Fleet", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "vehicle.assign-driver", module: "Fleet", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "vehicle.assignment.read", module: "Fleet", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: true },
    Permission { code: "vehicle.create", module: "Fleet", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "vehicle.end-assignment", module: "Fleet", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "vehicle.odometer.read", module: "Fleet", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: true },
    Permission { code: "vehicle.odometer.record", module: "Fleet", scope: Scope::Tenant, tenant_user: false, driver: true, mechanic: false, own: true },
    Permission { code: "vehicle.possession.read", module: "Fleet", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: true },
    Permission { code: "vehicle.read", module: "Fleet", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: true },
    Permission { code: "vehicle.tracking.read", module: "Fleet", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: true },
    Permission { code: "vehicle.update", module: "Fleet", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "fuel-entry.create", module: "Fuel", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: true },
    Permission { code: "fuel-entry.delete", module: "Fuel", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "fuel-entry.read", module: "Fuel", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: true },
    Permission { code: "fuel-entry.register-receipt", module: "Fuel", scope: Scope::Tenant, tenant_user: false, driver: true, mechanic: true, own: true },
    Permission { code: "fuel-entry.restore", module: "Fuel", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "fuel-entry.sync-provider", module: "Fuel", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "fuel-entry.unify", module: "Fuel", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "fuel-gauge-settings.read", module: "Fuel", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "fuel-gauge-settings.update", module: "Fuel", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "fuel.consumption-average.read", module: "Fuel", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "fuel.report.read", module: "Fuel", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "fuel.tank-gauge.read", module: "Fuel", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "internal-tank.read", module: "Fuel", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "internal-tank.stock.read", module: "Fuel", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "internal-tank.update", module: "Fuel", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "garage.call-list.read", module: "Garage", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "garage.call.cancel", module: "Garage", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "garage.call.create", module: "Garage", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "garage.device.manage", module: "Garage", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "garage.call.read", module: "Garage", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "garage.monitor-cards.read", module: "Garage", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "garage.monitor-map.read", module: "Garage", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "garage.monitor-matrix.read", module: "Garage", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "garage.monitor.read", module: "Garage", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "garage.queue.read", module: "Garage", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "garage.service-catalogue.create", module: "Garage", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "garage.service-catalogue.read", module: "Garage", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "garage.service-catalogue.update", module: "Garage", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "garage.service.external", module: "Garage", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "garage.service.internal", module: "Garage", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "garage.triage.check-out", module: "Garage", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "garage.triage.mark-service", module: "Garage", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "garage.triage.open", module: "Garage", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "garage.triage.read", module: "Garage", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "inspection-model.create", module: "Inspections", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "inspection-model.read", module: "Inspections", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "technical-inspection.create", module: "Inspections", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "technical-inspection.read", module: "Inspections", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "maintenance-notification.read", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "maintenance-notification.update", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "maintenance-plan.create", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "maintenance-plan.read", module: "Maintenance", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "non-execution.evaluate", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "non-execution.justify", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "non-execution.read", module: "Maintenance", scope: Scope::Tenant, tenant_user: true, driver: false, mechanic: true, own: false },
    Permission { code: "non-execution.record-manual", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "preventive-plan.alert.discharge", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "preventive-plan.alert.read", module: "Maintenance", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "preventive-plan.create", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "preventive-plan.extend", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "preventive-plan.open-work-order", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "preventive-plan.read", module: "Maintenance", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "preventive-plan.update", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "priced-service.create", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "priced-service.read", module: "Maintenance", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "service-type.create", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "service-type.read", module: "Maintenance", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "work-order.cancel", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "work-order.conclude", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "work-order.create", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "work-order.issue-part", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "work-order.item.add", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "work-order.item.resolve", module: "Maintenance", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "work-order.posting.read", module: "Maintenance", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "work-order.read", module: "Maintenance", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "business-plan.create", module: "Platform", scope: Scope::Platform, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "business-plan.delete", module: "Platform", scope: Scope::Platform, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "business-plan.read", module: "Platform", scope: Scope::Platform, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "business-plan.update", module: "Platform", scope: Scope::Platform, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "reference-data.city.create", module: "Reference data", scope: Scope::Platform, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "reference-data.city.import", module: "Reference data", scope: Scope::Platform, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "reference-data.city.read", module: "Reference data", scope: Scope::Baseline, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "reference-data.country.read", module: "Reference data", scope: Scope::Baseline, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "reference-data.province.create", module: "Reference data", scope: Scope::Platform, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "reference-data.province.import", module: "Reference data", scope: Scope::Platform, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "reference-data.province.manage-read", module: "Reference data", scope: Scope::Platform, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "reference-data.province.read", module: "Reference data", scope: Scope::Baseline, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "cost-report.supplier.read", module: "Reports", scope: Scope::Tenant, tenant_user: true, driver: false, mechanic: false, own: false },
    Permission { code: "cost-report.vehicle.read", module: "Reports", scope: Scope::Tenant, tenant_user: true, driver: false, mechanic: false, own: false },
    Permission { code: "cost-report.work-order.read", module: "Reports", scope: Scope::Tenant, tenant_user: true, driver: false, mechanic: false, own: false },
    Permission { code: "customer.create", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "customer.day-off.create", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "customer.day-off.delete", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "customer.day-off.read", module: "Scheduling", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "customer.read", module: "Scheduling", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "customer.update", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "extra-trip.create", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "extra-trip.import", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "extra-trip.import-preview", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "extra-trip.read", module: "Scheduling", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: true },
    Permission { code: "extra-trip.update", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "holiday.create", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "holiday.delete", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "holiday.read", module: "Scheduling", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "schedule.effective.read", module: "Scheduling", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "transport-demand.allocation.create", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "transport-demand.allocation.read", module: "Scheduling", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "transport-demand.allocation.update", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "transport-demand.create", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "transport-demand.daily-schedule.create", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "transport-demand.daily-schedule.delete", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "transport-demand.daily-schedule.read", module: "Scheduling", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "transport-demand.exception.create", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "transport-demand.exception.delete", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "transport-demand.exception.read", module: "Scheduling", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "transport-demand.read", module: "Scheduling", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "transport-demand.update", module: "Scheduling", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "part.create", module: "Stock & purchasing", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "part.read", module: "Stock & purchasing", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "part.stock-adjust", module: "Stock & purchasing", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "part.stock-entry", module: "Stock & purchasing", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "purchase-order.cancel", module: "Stock & purchasing", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "purchase-order.create", module: "Stock & purchasing", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "purchase-order.mark-ordered", module: "Stock & purchasing", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "purchase-order.mark-purchased", module: "Stock & purchasing", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "purchase-order.read", module: "Stock & purchasing", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "vehicle-expense.create", module: "Stock & purchasing", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "vehicle-expense.read", module: "Stock & purchasing", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "tenant-rule-settings.read", module: "Tenant", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "tenant-rule-settings.update", module: "Tenant", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "tenant.create", module: "Tenant", scope: Scope::Platform, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "tenant.plan", module: "Tenant", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "tenant.read", module: "Tenant", scope: Scope::Tenant, tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "tenant.update", module: "Tenant", scope: Scope::Tenant, tenant_user: false, driver: false, mechanic: false, own: false },
];

pub const OWN_SUFFIX: &str = "@own";

/// Keys of the five system profiles. They are seeded by the migration and their permissions re-derived
/// from this catalogue at every boot (`AccessProfileUseCase::sync_system_profiles`), so a permission
/// added here reaches the built-in profiles without a data migration.
pub const SYS_ADMIN: &str = "SysAdmin";
pub const TENANT_OWNER: &str = "TenantOwner";
pub const TENANT_USER: &str = "TenantUser";
pub const DRIVER: &str = "Driver";
pub const MECHANIC: &str = "Mechanic";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SystemProfile {
    pub key: &'static str,
    pub name: &'static str,
    pub scope: ProfileScope,
}

pub const SYSTEM_PROFILES: &[SystemProfile] = &[
    SystemProfile { key: SYS_ADMIN, name: "Platform administrator", scope: ProfileScope::Platform },
    SystemProfile { key: TENANT_OWNER, name: "Tenant owner", scope: ProfileScope::Tenant },
    SystemProfile { key: TENANT_USER, name: "Tenant user", scope: ProfileScope::Tenant },
    SystemProfile { key: DRIVER, name: "Driver", scope: ProfileScope::Tenant },
    SystemProfile { key: MECHANIC, name: "Mechanic", scope: ProfileScope::Tenant },
];

pub fn system_profile(key: &str) -> Option<&'static SystemProfile> {
    SYSTEM_PROFILES.iter().find(|p| p.key == key)
}

/// The system profiles below the owner -- the tenant's operational floor, which whoever administers
/// users may hand out without holding every grant in them (`C-035`); the owner's and the platform's are
/// guarded like any other profile.
pub fn is_operational_system_profile(profile: &crate::domain::access_profile::AccessProfile) -> bool {
    profile.system && matches!(profile.key.as_deref(), Some(TENANT_USER) | Some(DRIVER) | Some(MECHANIC))
}

/// The grants of a system profile, derived from the catalogue: the platform administrator holds every
/// tenant and platform permission, the owner every tenant permission, and the three below the owner
/// what their flag says -- the driver over their own records where the permission allows it.
pub fn system_grants(key: &str) -> Vec<String> {
    let mut grants: Vec<String> = CATALOGUE
        .iter()
        .filter(|p| p.scope != Scope::Baseline)
        .filter_map(|p| match key {
            SYS_ADMIN => Some(p.code.to_string()),
            TENANT_OWNER => (p.scope == Scope::Tenant).then(|| p.code.to_string()),
            TENANT_USER => p.tenant_user.then(|| p.code.to_string()),
            MECHANIC => p.mechanic.then(|| p.code.to_string()),
            DRIVER => p.driver.then(|| if p.own { format!("{}{}", p.code, OWN_SUFFIX) } else { p.code.to_string() }),
            _ => None,
        })
        .collect();
    grants.sort();
    grants
}

/// The system profiles whose grants hold `code` (fully or over own records) -- the catalogue's
/// `defaultFor` column.
pub fn default_for(code: &str) -> Vec<&'static str> {
    SYSTEM_PROFILES
        .iter()
        .filter(|p| p.scope == ProfileScope::Tenant && p.key != TENANT_OWNER)
        .filter(|p| Grants::from_codes(system_grants(p.key)).allows(code))
        .map(|p| p.key)
        .collect()
}

/// The permission an operation asks for (`HRMS-856`), by HTTP method and route template; `None` for the
/// public operations and any route not in the table.
pub fn for_operation(method: &str, path: &str) -> Option<&'static str> {
    crate::domain::operation_permission::OPERATIONS
        .iter()
        .find(|(m, p, _)| *m == method && *p == path)
        .map(|(_, _, code)| *code)
}

pub fn find(code: &str) -> Option<&'static Permission> {
    CATALOGUE.iter().find(|p| p.code == code)
}

/// What a profile of `scope` may be granted: tenant permissions for a tenant profile, those and the
/// platform's for a platform profile. The baseline is never granted.
pub fn grantable_in(scope: ProfileScope) -> impl Iterator<Item = &'static Permission> {
    CATALOGUE.iter().filter(move |p| match scope {
        ProfileScope::Tenant => p.scope == Scope::Tenant,
        ProfileScope::Platform => p.scope != Scope::Baseline,
    })
}

/// Splits a grant into its permission and whether it is own-scoped; `None` when the grant names no
/// catalogue permission, or asks `@own` of one that has no own scope.
pub fn parse_grant(grant: &str) -> Option<(&'static Permission, bool)> {
    let (code, own) = match grant.strip_suffix(OWN_SUFFIX) {
        Some(code) => (code, true),
        None => (grant, false),
    };
    let permission = find(code)?;
    (!own || permission.own).then_some((permission, own))
}

/// Normalises a list of grants for a profile of `scope`: every grant must parse and be grantable in that
/// scope; duplicates collapse and a full grant absorbs its own-scoped twin. `Err` names the offender.
pub fn normalise_grants(grants: &[String], scope: ProfileScope) -> Result<Vec<String>, String> {
    let mut held = Grants::default();
    for grant in grants {
        let Some((permission, own)) = parse_grant(grant) else {
            return Err(grant.clone());
        };
        let allowed = match scope {
            ProfileScope::Tenant => permission.scope == Scope::Tenant,
            ProfileScope::Platform => permission.scope != Scope::Baseline,
        };
        if !allowed {
            return Err(grant.clone());
        }
        held.grant(permission.code, if own { GrantScope::Own } else { GrantScope::Full });
    }
    Ok(held.codes())
}

/// How far a held permission reaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GrantScope {
    /// Over the holder's own records only (`code@own`).
    Own,
    /// Over every record the holder's tenant scope reaches.
    Full,
}

/// A resolved set of grants: permission code to the widest scope held. The baseline is included when
/// built through [`Grants::effective`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Grants {
    held: BTreeMap<String, GrantScope>,
}

impl Grants {
    /// Parses grants leniently (unknown codes are kept as full grants, so a catalogue rename never
    /// silently drops access); use [`normalise_grants`] to validate input.
    pub fn from_codes<I, S>(codes: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut grants = Self::default();
        for code in codes {
            let code = code.as_ref();
            match code.strip_suffix(OWN_SUFFIX) {
                Some(base) => grants.grant(base, GrantScope::Own),
                None => grants.grant(code, GrantScope::Full),
            }
        }
        grants
    }

    /// The caller's effective grants: the profile's, the user's own extras, and the baseline.
    pub fn effective(profile: &[String], extras: &[String]) -> Self {
        let mut grants = Self::from_codes(profile.iter().chain(extras.iter()));
        for p in CATALOGUE.iter().filter(|p| p.scope == Scope::Baseline) {
            grants.grant(p.code, GrantScope::Full);
        }
        grants
    }

    pub fn grant(&mut self, code: &str, scope: GrantScope) {
        let entry = self.held.entry(code.to_string()).or_insert(scope);
        if scope > *entry {
            *entry = scope;
        }
    }

    pub fn scope(&self, code: &str) -> Option<GrantScope> {
        self.held.get(code).copied()
    }

    /// Holds the permission at any scope.
    pub fn allows(&self, code: &str) -> bool {
        self.held.contains_key(code)
    }

    /// Holds the permission over the whole tenant scope, not only own records.
    pub fn allows_fully(&self, code: &str) -> bool {
        self.scope(code) == Some(GrantScope::Full)
    }

    /// Holds the permission, but over own records only.
    pub fn only_own(&self, code: &str) -> bool {
        self.scope(code) == Some(GrantScope::Own)
    }

    /// Every grant of `other` is held here at least as widely -- the rule that nobody grants what they do
    /// not hold (`C-035`).
    pub fn covers(&self, other: &Grants) -> bool {
        other.held.iter().all(|(code, scope)| self.scope(code).is_some_and(|mine| mine >= *scope))
    }

    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// Back to grant strings, sorted: `code` or `code@own`.
    pub fn codes(&self) -> Vec<String> {
        self.held
            .iter()
            .map(|(code, scope)| match scope {
                GrantScope::Full => code.clone(),
                GrantScope::Own => format!("{code}{OWN_SUFFIX}"),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for p in CATALOGUE {
            assert!(seen.insert(p.code), "duplicate {}", p.code);
        }
    }

    #[test]
    fn every_operation_asks_for_a_catalogue_permission() {
        use crate::domain::operation_permission::OPERATIONS;
        for (m, p, code) in OPERATIONS {
            assert!(find(code).is_some(), "{m} {p} asks for unknown permission {code}");
        }
        assert_eq!(for_operation("PUT", "/tenant/uuid/{uuid}").or(for_operation("PUT", "/tenant")), Some("tenant.update"));
    }

    #[test]
    fn the_driver_profile_carries_the_owners_rulings_over_own_records() {
        let driver = system_grants(DRIVER);
        for grant in ["checklist-run.create@own", "vehicle.odometer.record@own", "fuel-entry.register-receipt@own", DRIVER_ACT] {
            assert!(driver.contains(&grant.to_string()), "{grant}");
        }
        assert!(!driver.iter().any(|g| g.starts_with("purchase-order.create")));
    }

    #[test]
    fn a_mechanic_never_administers_and_the_owner_holds_every_tenant_permission() {
        let mechanic = Grants::from_codes(system_grants(MECHANIC));
        assert!(mechanic.allows("work-order.create") && mechanic.allows("part.stock-entry"));
        assert!(!mechanic.allows("user.create") && !mechanic.allows("tenant.update"));
        let owner = Grants::from_codes(system_grants(TENANT_OWNER));
        assert!(CATALOGUE.iter().filter(|p| p.scope == Scope::Tenant).all(|p| owner.allows_fully(p.code)));
        assert!(!owner.allows("tenant.create"));
        let platform = Grants::from_codes(system_grants(SYS_ADMIN));
        assert!(platform.allows("tenant.create") && platform.allows("vehicle.create"));
    }

    #[test]
    fn grants_parse_own_scope_and_a_full_grant_absorbs_it() {
        let grants = Grants::from_codes(["vehicle.read@own", "vehicle.read", "fuel-entry.read@own"]);
        assert!(grants.allows_fully("vehicle.read"));
        assert!(grants.only_own("fuel-entry.read"));
        assert!(!grants.allows("vehicle.create"));
        assert_eq!(grants.codes(), vec!["fuel-entry.read@own".to_string(), "vehicle.read".to_string()]);
    }

    #[test]
    fn normalising_refuses_unknown_codes_own_on_the_wrong_permission_and_platform_in_a_tenant() {
        assert!(normalise_grants(&["wizard.cast".into()], ProfileScope::Tenant).is_err());
        assert!(normalise_grants(&["vehicle.create@own".into()], ProfileScope::Tenant).is_err());
        assert!(normalise_grants(&["tenant.create".into()], ProfileScope::Tenant).is_err());
        assert!(normalise_grants(&["tenant.create".into()], ProfileScope::Platform).is_ok());
        assert!(normalise_grants(&["user.read-own".into()], ProfileScope::Platform).is_err());
        assert_eq!(
            normalise_grants(&["vehicle.read@own".into(), "vehicle.read".into()], ProfileScope::Tenant).unwrap(),
            vec!["vehicle.read".to_string()]
        );
    }

    #[test]
    fn covering_requires_at_least_the_same_scope() {
        let owner = Grants::from_codes(system_grants(TENANT_OWNER));
        let driver = Grants::from_codes(system_grants(DRIVER));
        assert!(owner.covers(&driver));
        assert!(!driver.covers(&owner));
        assert!(!Grants::from_codes(["vehicle.read@own"]).covers(&Grants::from_codes(["vehicle.read"])));
    }

    #[test]
    fn the_baseline_is_always_effective_and_default_for_names_the_built_in_profiles() {
        let grants = Grants::effective(&[], &[]);
        assert!(grants.allows("user.read-own") && grants.allows("reference-data.city.read"));
        assert_eq!(default_for("work-order.create"), vec![MECHANIC]);
        assert!(default_for("vehicle.read").contains(&DRIVER));
    }
}
