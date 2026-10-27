//! `C-032` (`D-27`, `HRMS-850`): the platform's catalogue of permissions -- one per resource and
//! action, grouped into modules only for navigation. Generated from
//! `02-system_requirements/hermes/access-model_permission_catalogue.md`; edit that, not this.
//!
//! Three kinds, by what a profile can do with them:
//! - [`GRANTABLE`]: what a tenant owner puts in a profile. The `tenant_user` / `driver` / `mechanic`
//!   flags are today's behaviour for that role -- the built-in default profile (`HRMS-854`).
//! - [`OWNER_ONLY`]: the tenant's record and configuration, user administration. Never in a profile.
//! - [`BASELINE`]: every authenticated user, always.

use crate::domain::enums::Role;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Permission {
    pub code: &'static str,
    pub module: &'static str,
    pub tenant_user: bool,
    pub driver: bool,
    pub mechanic: bool,
    /// May be granted over the user's own records only (`AC-04`).
    pub own: bool,
}

pub const GRANTABLE: &[Permission] = &[
    Permission { code: "checklist-run.create", module: "Checklists", tenant_user: false, driver: true, mechanic: false, own: true },
    Permission { code: "checklist-run.read", module: "Checklists", tenant_user: true, driver: true, mechanic: true, own: true },
    Permission { code: "checklist-template.create", module: "Checklists", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "checklist-template.read", module: "Checklists", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "vehicle-presence.read", module: "Fleet", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "vehicle.assign-driver", module: "Fleet", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "vehicle.assignment.read", module: "Fleet", tenant_user: true, driver: true, mechanic: true, own: true },
    Permission { code: "vehicle.create", module: "Fleet", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "vehicle.end-assignment", module: "Fleet", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "vehicle.odometer.read", module: "Fleet", tenant_user: true, driver: true, mechanic: true, own: true },
    Permission { code: "vehicle.odometer.record", module: "Fleet", tenant_user: false, driver: true, mechanic: false, own: true },
    Permission { code: "vehicle.possession.read", module: "Fleet", tenant_user: true, driver: true, mechanic: true, own: true },
    Permission { code: "vehicle.read", module: "Fleet", tenant_user: true, driver: true, mechanic: true, own: true },
    Permission { code: "vehicle.tracking.read", module: "Fleet", tenant_user: true, driver: true, mechanic: true, own: true },
    Permission { code: "vehicle.update", module: "Fleet", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "fuel-entry.create", module: "Fuel", tenant_user: false, driver: false, mechanic: true, own: true },
    Permission { code: "fuel-entry.delete", module: "Fuel", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "fuel-entry.read", module: "Fuel", tenant_user: true, driver: true, mechanic: true, own: true },
    Permission { code: "fuel-entry.register-receipt", module: "Fuel", tenant_user: false, driver: true, mechanic: true, own: true },
    Permission { code: "fuel-entry.restore", module: "Fuel", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "fuel-entry.sync-provider", module: "Fuel", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "fuel-entry.unify", module: "Fuel", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "fuel.consumption-average.read", module: "Fuel", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "cost-report.supplier.read", module: "Reports", tenant_user: true, driver: false, mechanic: false, own: false },
    Permission { code: "cost-report.vehicle.read", module: "Reports", tenant_user: true, driver: false, mechanic: false, own: false },
    Permission { code: "cost-report.work-order.read", module: "Reports", tenant_user: true, driver: false, mechanic: false, own: false },
    Permission { code: "fuel.report.read", module: "Fuel", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "fuel.tank-gauge.read", module: "Fuel", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "internal-tank.read", module: "Fuel", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "internal-tank.stock.read", module: "Fuel", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "garage.call-list.read", module: "Garage", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "garage.call.cancel", module: "Garage", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "garage.call.create", module: "Garage", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "garage.call.read", module: "Garage", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "garage.monitor-cards.read", module: "Garage", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "garage.monitor-map.read", module: "Garage", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "garage.monitor-matrix.read", module: "Garage", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "garage.monitor.read", module: "Garage", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "garage.queue.read", module: "Garage", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "garage.service-catalogue.create", module: "Garage", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "garage.service-catalogue.read", module: "Garage", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "garage.service-catalogue.update", module: "Garage", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "garage.triage.check-out", module: "Garage", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "garage.service.external", module: "Garage", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "garage.service.internal", module: "Garage", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "garage.triage.mark-service", module: "Garage", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "garage.triage.open", module: "Garage", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "garage.triage.read", module: "Garage", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "inspection-model.create", module: "Inspections", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "inspection-model.read", module: "Inspections", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "technical-inspection.create", module: "Inspections", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "technical-inspection.read", module: "Inspections", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "maintenance-notification.read", module: "Maintenance", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "maintenance-notification.update", module: "Maintenance", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "non-execution.justify", module: "Maintenance", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "non-execution.read", module: "Maintenance", tenant_user: true, driver: false, mechanic: true, own: false },
    Permission { code: "non-execution.record-manual", module: "Maintenance", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "maintenance-plan.create", module: "Maintenance", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "maintenance-plan.read", module: "Maintenance", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "preventive-plan.alert.discharge", module: "Maintenance", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "preventive-plan.alert.read", module: "Maintenance", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "preventive-plan.create", module: "Maintenance", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "preventive-plan.extend", module: "Maintenance", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "preventive-plan.open-work-order", module: "Maintenance", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "preventive-plan.read", module: "Maintenance", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "preventive-plan.update", module: "Maintenance", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "priced-service.create", module: "Maintenance", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "priced-service.read", module: "Maintenance", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "service-type.create", module: "Maintenance", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "service-type.read", module: "Maintenance", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "work-order.cancel", module: "Maintenance", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "work-order.conclude", module: "Maintenance", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "work-order.create", module: "Maintenance", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "work-order.issue-part", module: "Maintenance", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "work-order.item.add", module: "Maintenance", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "work-order.posting.read", module: "Maintenance", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "work-order.read", module: "Maintenance", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "customer.create", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "customer.day-off.create", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "customer.day-off.delete", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "customer.day-off.read", module: "Scheduling", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "customer.read", module: "Scheduling", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "customer.update", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "extra-trip.create", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "extra-trip.import", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "extra-trip.import-preview", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "extra-trip.read", module: "Scheduling", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "extra-trip.update", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "holiday.create", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "holiday.delete", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "holiday.read", module: "Scheduling", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "schedule.effective.read", module: "Scheduling", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "transport-demand.allocation.create", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "transport-demand.allocation.read", module: "Scheduling", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "transport-demand.allocation.update", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "transport-demand.create", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "transport-demand.daily-schedule.create", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "transport-demand.daily-schedule.delete", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "transport-demand.daily-schedule.read", module: "Scheduling", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "transport-demand.exception.create", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "transport-demand.exception.delete", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "transport-demand.exception.read", module: "Scheduling", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "transport-demand.read", module: "Scheduling", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "transport-demand.update", module: "Scheduling", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "part.create", module: "Stock & purchasing", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "part.read", module: "Stock & purchasing", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "part.stock-adjust", module: "Stock & purchasing", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "part.stock-entry", module: "Stock & purchasing", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "purchase-order.cancel", module: "Stock & purchasing", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "purchase-order.create", module: "Stock & purchasing", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "purchase-order.mark-ordered", module: "Stock & purchasing", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "purchase-order.mark-purchased", module: "Stock & purchasing", tenant_user: false, driver: false, mechanic: true, own: false },
    Permission { code: "purchase-order.read", module: "Stock & purchasing", tenant_user: true, driver: true, mechanic: true, own: false },
    Permission { code: "vehicle-expense.create", module: "Stock & purchasing", tenant_user: false, driver: false, mechanic: false, own: false },
    Permission { code: "vehicle-expense.read", module: "Stock & purchasing", tenant_user: true, driver: true, mechanic: true, own: false },
];

pub const OWNER_ONLY: &[&str] = &[
    "billing.read",
    "fuel-gauge-settings.read",
    "fuel-gauge-settings.update",
    "internal-tank.update",
    "non-execution.evaluate",
    "profile.assign",
    "profile.create",
    "profile.read",
    "profile.update",
    "tenant-rule-settings.read",
    "tenant-rule-settings.update",
    "tenant.plan",
    "tenant.read",
    "tenant.update",
    "user.create",
    "user.invite",
    "user.update",
];

/// The platform administrator's alone; never in a profile.
pub const PLATFORM: &[&str] = &[
    "billing.confirm-payment",
    "billing.configure",
    "billing.suspend",
    "business-plan.create",
    "business-plan.delete",
    "business-plan.read",
    "business-plan.update",
    "reference-data.city.create",
    "reference-data.city.import",
    "reference-data.province.create",
    "reference-data.province.import",
    "reference-data.province.manage-read",
    "tenant.create",
    "vehicle-presence.publish",
];

pub const BASELINE: &[&str] = &[
    "reference-data.city.read",
    "reference-data.country.read",
    "reference-data.province.read",
    "user.change-own-password",
    "user.read-own",
];

/// The permission an operation asks for (`HRMS-856`), by HTTP method and route template; `None` for the
/// public operations and any route not in the table.
pub fn for_operation(method: &str, path: &str) -> Option<&'static str> {
    crate::domain::operation_permission::OPERATIONS
        .iter()
        .find(|(m, p, _)| *m == method && *p == path)
        .map(|(_, _, code)| *code)
}

pub fn find(code: &str) -> Option<&'static Permission> {
    GRANTABLE.iter().find(|p| p.code == code)
}

/// The built-in default profile of a role: exactly what the system allowed that role before profiles
/// existed, so introducing profiles changes nothing for a user who has none.
pub fn default_permissions(role: &Role) -> Vec<&'static str> {
    GRANTABLE
        .iter()
        .filter(|p| match role {
            Role::TenantUser => p.tenant_user,
            Role::Driver => p.driver,
            Role::Mechanic => p.mechanic,
            Role::TenantOwner | Role::SysAdmin => true,
        })
        .map(|p| p.code)
        .collect()
}

/// Roles that hold a profile. The owner always holds everything; the platform administrator is unbound.
pub fn takes_profile(role: &Role) -> bool {
    matches!(role, Role::TenantUser | Role::Driver | Role::Mechanic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique_and_never_both_grantable_and_fixed() {
        let mut seen = std::collections::HashSet::new();
        for p in GRANTABLE {
            assert!(seen.insert(p.code), "duplicate {}", p.code);
            assert!(!OWNER_ONLY.contains(&p.code) && !BASELINE.contains(&p.code), "{}", p.code);
        }
    }

    #[test]
    fn every_operation_asks_for_a_catalogue_permission() {
        use crate::domain::operation_permission::OPERATIONS;
        for (m, p, code) in OPERATIONS {
            let known = find(code).is_some()
                || OWNER_ONLY.contains(code)
                || BASELINE.contains(code)
                || PLATFORM.contains(code);
            assert!(known, "{m} {p} asks for unknown permission {code}");
        }
        assert_eq!(for_operation("PUT", "/tenant/uuid/{uuid}").or(for_operation("PUT", "/tenant")), Some("tenant.update"));
    }

    #[test]
    fn the_driver_default_carries_the_owners_rulings() {
        let driver = default_permissions(&Role::Driver);
        for code in ["checklist-run.create", "vehicle.odometer.record", "fuel-entry.register-receipt"] {
            assert!(driver.contains(&code), "{code}");
        }
        assert!(!driver.contains(&"purchase-order.create"));
    }

    #[test]
    fn a_mechanic_default_is_not_wider_than_the_catalogue_and_never_administers() {
        let mechanic = default_permissions(&Role::Mechanic);
        assert!(mechanic.contains(&"work-order.create") && mechanic.contains(&"part.stock-entry"));
        assert!(mechanic.iter().all(|c| !OWNER_ONLY.contains(c)));
    }

    #[test]
    fn only_tenant_roles_below_the_owner_take_a_profile() {
        assert!(takes_profile(&Role::Driver) && takes_profile(&Role::Mechanic) && takes_profile(&Role::TenantUser));
        assert!(!takes_profile(&Role::TenantOwner) && !takes_profile(&Role::SysAdmin));
    }
}
