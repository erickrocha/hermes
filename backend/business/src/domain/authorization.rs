//! EPIC-XF-02 (HRMS-012...017): the one shared authorization module AD-016 calls for.
//!
//! `C-035` rebuilt it on permissions. Before, every capability here was a role test (`actor.role ==
//! Role::TenantOwner && ...`) and the profile a user held was consulted only by the middleware, only for
//! the three roles below the owner. Now a capability is two questions and nothing else:
//!
//! 1. **Does the actor hold the permission?** -- `User::access`, resolved by `UserGateway` from the
//!    actor's profile, their own extra grants and the baseline (`domain::permission`).
//! 2. **Does the actor's tenant reach the record?** -- a platform actor (a `ProfileScope::Platform`
//!    profile, no tenant; PD-013's "unbound administrator" by another name) reaches every tenant; a
//!    tenant actor reaches their own tenant only (HRM-090...096).
//!
//! The named `can_*` functions stay, so a call site reads as a business rule and a test can pin one
//! down; each is a one-line composition of [`may_act_on`] / [`may_create_in`] / [`may`] over the
//! catalogue code the operation asks for (`operation_permission::OPERATIONS`). The middleware asks the
//! same question for every operation before the handler runs (`HRMS-856`); the handler asks it again
//! with the record's tenant in hand.
//!
//! Own-records scope (`code@own`, `AC-04`) is not decided here: a handler that narrows a listing to the
//! caller's own records asks `User::only_own(code)`.

use crate::domain::access_profile::AccessProfile;
use crate::domain::enums::ProfileScope;
use crate::domain::permission::{CHILD_CREATE, Grants, DRIVER_ACT};
use crate::domain::user::User;

/// PD-013, restated for `C-035` and again for `C-036` (`D-37`): the platform actor holds a platform profile
/// and belongs to the root tenant, the parent of every other (`HRMS-874`). Reaching every tenant is therefore
/// the same as reaching the root's descendants.
pub fn is_platform_actor(user: &User) -> bool {
    user.access.scope == ProfileScope::Platform && user.tenant_id.is_some()
}

/// HRMS-111/HRMS-133: a tenant profile is held inside a tenant. An account that breaks that -- or a
/// platform profile stamped with a tenant -- is refused outright rather than treated as unscoped.
pub fn lacks_required_tenant(user: &User) -> bool {
    user.tenant_id.is_none()
}

/// Whether `user` may read or act on the given tenant's own data: a platform actor (any tenant), or a
/// member of that exact tenant (HRM-090...096).
pub fn can_access_tenant(user: &User, tenant_id: i64) -> bool {
    user.tenant_id == Some(tenant_id) || is_platform_actor(user)
}

/// The actor's tenant reaches a record stamped with `record_tenant_id`.
pub fn reaches(actor: &User, record_tenant_id: Option<i64>) -> bool {
    is_platform_actor(actor) || (actor.tenant_id.is_some() && actor.tenant_id == record_tenant_id)
}

/// Holds `code` and reaches the record's tenant.
pub fn may_act_on(actor: &User, code: &str, record_tenant_id: Option<i64>) -> bool {
    actor.allows(code) && reaches(actor, record_tenant_id)
}

/// Holds any of `codes` and reaches the record's tenant -- for a handler that guards a family of
/// operations the middleware already told apart one by one.
pub fn may_act_on_any(actor: &User, codes: &[&str], record_tenant_id: Option<i64>) -> bool {
    codes.iter().any(|code| actor.allows(code)) && reaches(actor, record_tenant_id)
}

/// Holds `code`, for an operation with no record to compare tenants against. A tenant actor without a
/// tenant holds nothing.
pub fn may(actor: &User, code: &str) -> bool {
    actor.allows(code) && (is_platform_actor(actor) || actor.tenant_id.is_some())
}

/// Holds `code` and may create inside `target_tenant_id`: a platform actor names the owning tenant; a
/// tenant actor creates in their own tenant only (an omitted target defaults to it).
pub fn may_create_in(actor: &User, code: &str, target_tenant_id: Option<i64>) -> bool {
    if !actor.allows(code) {
        return false;
    }
    if is_platform_actor(actor) {
        return target_tenant_id.is_some();
    }
    actor.tenant_id.is_some() && (target_tenant_id.is_none() || target_tenant_id == actor.tenant_id)
}

/// `C-035`: nobody grants what they do not hold -- a profile's grants, or a user's extras, must be
/// covered by the actor's own effective grants, at least as widely (`@own` does not cover full).
pub fn can_grant(actor: &User, grants: &Grants) -> bool {
    actor.access.grants.covers(grants)
}

/// Whether a profile may be held by an account in `tenant_id`: a platform profile by an account with no
/// tenant; a tenant profile inside the tenant that owns it, or inside any tenant for a system profile.
pub fn profile_fits(profile: &AccessProfile, tenant_id: Option<i64>) -> bool {
    profile_fits_in(profile, tenant_id, &[])
}

/// A profile a tenant above the account's own made is visible and assignable below it (`D-37`, `HRMS-873`):
/// `ancestors` are the tenants above `tenant_id`.
pub fn profile_fits_in(profile: &AccessProfile, tenant_id: Option<i64>, ancestors: &[i64]) -> bool {
    // Where the system administrator's profile may sit (the root tenant, the placing actor's own) is pinned by
    // `can_assign_profile` and `AccessProfileUseCase::placeable`.
    profile.active && tenant_id.is_some() && (profile.system || profile.tenant_id == tenant_id || inherited(profile, ancestors))
}

fn inherited(profile: &AccessProfile, ancestors: &[i64]) -> bool {
    profile.tenant_id.is_some_and(|owner| ancestors.contains(&owner))
}

/// `C-035`: whether `user` may be named as a driver -- of a vehicle, a trip, a checklist, a fuelling --
/// inside `tenant_id`. The former `role == Driver` test, now the `driver.act` permission.
pub fn is_active_driver_of(user: &User, tenant_id: Option<i64>) -> bool {
    user.enabled && user.tenant_id.is_some() && user.tenant_id == tenant_id && user.allows(DRIVER_ACT)
}

// ---------------------------------------------------------------------------------------------------
// Tenant record and configuration
// ---------------------------------------------------------------------------------------------------

/// `PUT /tenant/uuid/{uuid}` (`D-27` 4, 8; `C-032`).
pub fn can_update_tenant(user: &User, tenant_id: i64) -> bool {
    may_act_on(user, "tenant.update", Some(tenant_id))
}

/// `D-27` 8 (`HRMS-857`): the tenant's full record and its configuration (rule and gauge settings) are
/// read by whoever holds the configuration permission; everyone else gets the summary the console themes
/// itself from.
pub fn can_read_tenant_configuration(user: &User) -> bool {
    may(user, "tenant-rule-settings.read")
}

/// Billing: the tenant's charges and settings are read by whoever holds `billing.read`; the platform
/// configures, suspends and confirms (platform permissions).
pub fn can_read_billing(user: &User) -> bool {
    may(user, "billing.read")
}

/// `POST /tenant` -- the platform's alone (PD-019).
pub fn can_create_tenant(user: &User) -> bool {
    user.tenant_id.is_some() && user.allows(CHILD_CREATE)
}

/// `POST /tenant/{id}/plan` (HRMS-224, PD-019, PD-021).
pub fn can_set_tenant_plan(user: &User) -> bool {
    is_platform_actor(user) && user.allows("tenant.plan")
}

/// The business-plan catalogue is platform-global (PD-014).
pub fn can_manage_business_plan_catalogue(user: &User) -> bool {
    is_platform_actor(user)
        && may_act_on_any(
            user,
            &["business-plan.create", "business-plan.update", "business-plan.delete", "business-plan.read"],
            None,
        )
}

/// PD-027: reference data (provinces, cities) is platform-global, exactly like the business-plan catalogue.
pub fn can_manage_reference_data(user: &User) -> bool {
    is_platform_actor(user)
        && [
            "reference-data.province.create",
            "reference-data.province.import",
            "reference-data.city.create",
            "reference-data.city.import",
            "reference-data.province.manage-read",
        ]
        .iter()
        .any(|code| user.allows(code))
}

/// `C-030` `HRMS-804…810`: what a vehicle costs and earns is the office's to read.
pub fn can_read_cost_reports(user: &User) -> bool {
    ["cost-report.vehicle.read", "cost-report.supplier.read", "cost-report.work-order.read"]
        .iter()
        .any(|code| may(user, code))
}

// ---------------------------------------------------------------------------------------------------
// Access profiles and users
// ---------------------------------------------------------------------------------------------------

pub const PROFILE_ADMIN: &[&str] = &["profile.read", "profile.create", "profile.update", "profile.assign"];
pub const USER_ADMIN: &[&str] = &["user.create", "user.update", "user.invite", "profile.assign"];

/// `C-032` (`HRMS-855`): profiles and who holds them. `tenant_id` is the profile's owner -- `None` for a
/// system or platform profile, which every tenant reads and only the platform edits (the use case keeps
/// system profiles read-only).
pub fn can_manage_access_profiles(actor: &User, tenant_id: Option<i64>) -> bool {
    PROFILE_ADMIN.iter().any(|code| actor.allows(code))
        && (is_platform_actor(actor) || (actor.tenant_id.is_some() && (tenant_id.is_none() || tenant_id == actor.tenant_id)))
}

/// Where a newly created user's tenant comes from, once [`can_assign_profile`] has decided the
/// placement is allowed at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationTenant {
    /// Fixed by the profile and the actor, not the caller's choice: `None` for a platform profile,
    /// `Some(id)` for a tenant actor placing someone in their own tenant (HRMS-115, HRMS-118).
    Fixed(Option<i64>),
    /// The caller names the tenant (a platform actor giving out a tenant profile, HRMS-114). The caller
    /// of this function is responsible for validating that the named tenant actually exists -- this
    /// module has no I/O and cannot do that itself.
    CallerChosen,
}

/// PD-019's hierarchy, restated for profiles (`C-035`): a platform actor hands out platform profiles
/// (to accounts with no tenant) and tenant profiles (naming the tenant); a tenant actor hands out
/// tenant profiles -- the system ones or their own tenant's -- inside their own tenant only. Holding the
/// permission for the operation (`user.create`, `user.update`, `profile.assign`) is the middleware's
/// question; covering the profile's grants is [`can_grant`]'s.
/// The *Administrador do Sistema* system profile.
pub fn is_system_administrator(profile: &AccessProfile) -> bool {
    profile.system && profile.key.as_deref() == Some(crate::domain::permission::SYS_ADMIN)
}

pub fn can_assign_profile(actor: &User, profile: &AccessProfile) -> Option<CreationTenant> {
    can_assign_profile_in(actor, profile, &[])
}

/// [`can_assign_profile`] with the tenants above the actor's own, whose profiles are theirs to hand out too.
pub fn can_assign_profile_in(actor: &User, profile: &AccessProfile, ancestors: &[i64]) -> Option<CreationTenant> {
    if !profile.active {
        return None;
    }
    let administrator = is_system_administrator(profile);
    if is_platform_actor(actor) {
        return Some(if administrator { CreationTenant::Fixed(actor.tenant_id) } else { CreationTenant::CallerChosen });
    }
    let tenant_id = actor.tenant_id?;
    (!administrator && (profile.system || profile.tenant_id == Some(tenant_id) || inherited(profile, ancestors)))
        .then_some(CreationTenant::Fixed(Some(tenant_id)))
}

/// Whether `actor` may move an *existing* account to another tenant while editing it (PD-019): the
/// platform's decision alone, and only onto a tenant that fits the account's profile.
pub fn can_move_user_to_tenant(actor: &User, profile: &AccessProfile, requested_tenant_id: Option<i64>) -> bool {
    is_platform_actor(actor)
        && profile_fits(profile, requested_tenant_id)
        && (!is_system_administrator(profile) || requested_tenant_id == actor.tenant_id)
}

/// Whether `actor` may *read* `target`'s record (EPIC-IA-05-S01): the target themselves, or whoever
/// administers users in the target's tenant.
pub fn can_read_user_record(actor: &User, target_id: Option<i64>, target_tenant_id: Option<i64>) -> bool {
    (actor.id.is_some() && actor.id == target_id) || can_administer_user(actor, target_id, target_tenant_id)
}

/// Whether `actor` may administer `target`'s record -- edit it, invite them, change their profile
/// (EPIC-IA-05-S01, HRMS-117, HRMS-118). DEF-IA-03: being the target is not enough; what a user may do
/// to their own account without administering it has its own route.
pub fn can_administer_user(actor: &User, _target_id: Option<i64>, target_tenant_id: Option<i64>) -> bool {
    may_act_on_any(actor, USER_ADMIN, target_tenant_id)
}

/// `GET /user`: who sees the user list at all -- whoever administers users; everyone else an empty page.
pub fn can_list_users(actor: &User) -> bool {
    USER_ADMIN.iter().any(|code| may(actor, code))
}

// ---------------------------------------------------------------------------------------------------
// Fleet
// ---------------------------------------------------------------------------------------------------

/// EPIC-FO-01-S04 (HRMS-923, PD-019): who may register, edit or retire a vehicle, assign and end a driver.
pub fn can_administer_vehicle(actor: &User, vehicle_tenant_id: Option<i64>) -> bool {
    may_act_on_any(actor, &["vehicle.update", "vehicle.assign-driver", "vehicle.end-assignment"], vehicle_tenant_id)
}

pub fn can_create_vehicle(actor: &User, target_tenant_id: Option<i64>) -> bool {
    may_create_in(actor, "vehicle.create", target_tenant_id)
}

/// EPIC-FO-01-S04/S05 (HRMS-923, HRMS-924): reading a vehicle is wider than administering it. Cross-tenant
/// is no visibility at all, which is why `vehicle_endpoint` answers 404 there and not 403 (PD-034).
pub fn can_read_vehicle(actor: &User, vehicle_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "vehicle.read", vehicle_tenant_id)
}

/// EPIC-FO-02-S01/S02 (HRMS-926, HRMS-927): linking a tracking device is the platform's act -- the
/// provider account is the platform's.
pub fn can_link_tracker_device(actor: &User) -> bool {
    is_platform_actor(actor) && actor.allows("vehicle.update")
}

/// `C-032` (`HRMS-860`): recording the odometer; a driver holds it over their own vehicle (reached only
/// through `find_visible`, which scopes an own-records holder to their assignment).
pub fn can_record_odometer(actor: &User, vehicle_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "vehicle.odometer.record", vehicle_tenant_id)
}

// ---------------------------------------------------------------------------------------------------
// Scheduling
// ---------------------------------------------------------------------------------------------------

pub fn can_administer_customer(actor: &User, customer_tenant_id: Option<i64>) -> bool {
    may_act_on_any(actor, &["customer.update", "customer.day-off.create", "customer.day-off.delete"], customer_tenant_id)
}

pub fn can_create_customer(actor: &User, target_tenant_id: Option<i64>) -> bool {
    may_create_in(actor, "customer.create", target_tenant_id)
}

pub fn can_read_customer(actor: &User, customer_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "customer.read", customer_tenant_id)
}

pub fn can_administer_holiday(actor: &User, holiday_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "holiday.delete", holiday_tenant_id)
}

pub fn can_create_holiday(actor: &User, target_tenant_id: Option<i64>) -> bool {
    may_create_in(actor, "holiday.create", target_tenant_id)
}

pub fn can_read_holiday(actor: &User, holiday_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "holiday.read", holiday_tenant_id)
}

/// `EPIC-SC-02-S01` (`HRMS-603`, `C-024`): editing a demand and everything hung off it -- allocations,
/// daily schedules, exceptions.
pub fn can_administer_transport_demand(actor: &User, demand_tenant_id: Option<i64>) -> bool {
    may_act_on_any(
        actor,
        &[
            "transport-demand.update",
            "transport-demand.allocation.create",
            "transport-demand.allocation.update",
            "transport-demand.daily-schedule.create",
            "transport-demand.daily-schedule.delete",
            "transport-demand.exception.create",
            "transport-demand.exception.delete",
        ],
        demand_tenant_id,
    )
}

pub fn can_create_transport_demand(actor: &User, target_tenant_id: Option<i64>) -> bool {
    may_create_in(actor, "transport-demand.create", target_tenant_id)
}

pub fn can_read_transport_demand(actor: &User, demand_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "transport-demand.read", demand_tenant_id)
}

pub fn can_administer_extra_trip(actor: &User, trip_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "extra-trip.update", trip_tenant_id)
}

pub fn can_create_extra_trip(actor: &User, target_tenant_id: Option<i64>) -> bool {
    may_create_in(actor, "extra-trip.create", target_tenant_id)
}

pub fn can_read_extra_trip(actor: &User, trip_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "extra-trip.read", trip_tenant_id)
}

// ---------------------------------------------------------------------------------------------------
// Checklists
// ---------------------------------------------------------------------------------------------------

pub fn can_administer_checklist_template(actor: &User, template_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "checklist-template.create", template_tenant_id)
}

pub fn can_create_checklist_template(actor: &User, target_tenant_id: Option<i64>) -> bool {
    may_create_in(actor, "checklist-template.create", target_tenant_id)
}

pub fn can_read_checklist_template(actor: &User, template_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "checklist-template.read", template_tenant_id)
}

pub fn can_administer_checklist_run(actor: &User, run_tenant_id: Option<i64>) -> bool {
    actor.allows_fully("checklist-run.create") && reaches(actor, run_tenant_id)
}

/// `C-032` (`D-27` 5): submitting the checklist is the driver's central act -- on their own vehicle and
/// as themselves (`checklist-run.create@own`), which the handler enforces.
pub fn can_create_checklist_run(actor: &User, target_tenant_id: Option<i64>) -> bool {
    may_create_in(actor, "checklist-run.create", target_tenant_id)
}

pub fn can_read_checklist_run(actor: &User, run_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "checklist-run.read", run_tenant_id)
}

// ---------------------------------------------------------------------------------------------------
// Maintenance
// ---------------------------------------------------------------------------------------------------

/// `EPIC-MT-01-S01` (`HRMS-700`, `C-026`): closing and cancelling a work order.
pub fn can_administer_work_order(actor: &User, work_order_tenant_id: Option<i64>) -> bool {
    may_act_on_any(actor, &["work-order.cancel", "work-order.conclude"], work_order_tenant_id)
}

/// Opening a work order and working it: items, parts, resolutions.
pub fn can_create_work_order(actor: &User, target_tenant_id: Option<i64>) -> bool {
    ["work-order.create", "work-order.item.add", "work-order.item.resolve", "work-order.issue-part"]
        .iter()
        .any(|code| may_create_in(actor, code, target_tenant_id))
}

pub fn can_read_work_order(actor: &User, work_order_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "work-order.read", work_order_tenant_id)
}

pub fn can_administer_maintenance_plan(actor: &User, plan_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "maintenance-plan.create", plan_tenant_id)
}

pub fn can_create_maintenance_plan(actor: &User, target_tenant_id: Option<i64>) -> bool {
    may_create_in(actor, "maintenance-plan.create", target_tenant_id)
}

pub fn can_read_maintenance_plan(actor: &User, plan_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "maintenance-plan.read", plan_tenant_id)
}

pub fn can_administer_service_type(actor: &User, service_type_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "service-type.create", service_type_tenant_id)
}

pub fn can_create_service_type(actor: &User, target_tenant_id: Option<i64>) -> bool {
    may_create_in(actor, "service-type.create", target_tenant_id)
}

pub fn can_read_service_type(actor: &User, service_type_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "service-type.read", service_type_tenant_id)
}

pub fn can_administer_priced_service(actor: &User, priced_service_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "priced-service.create", priced_service_tenant_id)
}

pub fn can_create_priced_service(actor: &User, target_tenant_id: Option<i64>) -> bool {
    may_create_in(actor, "priced-service.create", target_tenant_id)
}

pub fn can_read_priced_service(actor: &User, priced_service_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "priced-service.read", priced_service_tenant_id)
}

/// `EPIC-MT-07-S01` (`HRMS-706`, `C-027`): preventive plans, inspection models and technical inspections
/// are maintenance work -- one family, one gate.
pub fn can_create_preventive_plan(actor: &User, target_tenant_id: Option<i64>) -> bool {
    [
        "preventive-plan.create",
        "preventive-plan.update",
        "preventive-plan.extend",
        "preventive-plan.open-work-order",
        "preventive-plan.alert.discharge",
        "inspection-model.create",
        "technical-inspection.create",
    ]
    .iter()
    .any(|code| may_create_in(actor, code, target_tenant_id))
}

pub fn can_read_preventive_plan(actor: &User, preventive_plan_tenant_id: Option<i64>) -> bool {
    may_act_on_any(
        actor,
        &["preventive-plan.read", "preventive-plan.alert.read", "inspection-model.read", "technical-inspection.read"],
        preventive_plan_tenant_id,
    )
}

/// `EPIC-MT-04` (`C-026`): the non-execution inbox.
pub fn can_read_non_execution(actor: &User) -> bool {
    may(actor, "non-execution.read")
}

/// `EPIC-MT-04` (`C-026`, `SR-MT-04-12`): triggering an evaluation.
pub fn can_evaluate_non_execution(actor: &User) -> bool {
    may(actor, "non-execution.evaluate")
}

/// Justifying a non-execution and recording one by hand; reading and marking maintenance notifications.
pub fn can_work_maintenance(actor: &User, tenant_id: Option<i64>) -> bool {
    may_act_on_any(
        actor,
        &[
            "non-execution.justify",
            "non-execution.record-manual",
            "maintenance-notification.read",
            "maintenance-notification.update",
        ],
        tenant_id,
    )
}

// ---------------------------------------------------------------------------------------------------
// Stock, purchasing and expenses
// ---------------------------------------------------------------------------------------------------

pub fn can_administer_part(actor: &User, part_tenant_id: Option<i64>) -> bool {
    may_act_on_any(actor, &["part.create", "part.stock-entry", "part.stock-adjust"], part_tenant_id)
}

pub fn can_create_part(actor: &User, target_tenant_id: Option<i64>) -> bool {
    may_create_in(actor, "part.create", target_tenant_id)
}

pub fn can_read_part(actor: &User, part_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "part.read", part_tenant_id)
}

pub fn can_create_stock_movement(actor: &User, target_tenant_id: Option<i64>) -> bool {
    ["part.stock-entry", "part.stock-adjust"].iter().any(|code| may_create_in(actor, code, target_tenant_id))
}

pub fn can_create_purchase_order(actor: &User, target_tenant_id: Option<i64>) -> bool {
    may_create_in(actor, "purchase-order.create", target_tenant_id)
}

pub fn can_administer_purchase_order(actor: &User, purchase_order_tenant_id: Option<i64>) -> bool {
    may_act_on_any(
        actor,
        &["purchase-order.mark-ordered", "purchase-order.mark-purchased", "purchase-order.cancel"],
        purchase_order_tenant_id,
    )
}

pub fn can_read_purchase_order(actor: &User, purchase_order_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "purchase-order.read", purchase_order_tenant_id)
}

pub fn can_administer_vehicle_expense(actor: &User, vehicle_expense_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "vehicle-expense.create", vehicle_expense_tenant_id)
}

pub fn can_create_vehicle_expense(actor: &User, target_tenant_id: Option<i64>) -> bool {
    may_create_in(actor, "vehicle-expense.create", target_tenant_id)
}

pub fn can_read_vehicle_expense(actor: &User, vehicle_expense_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "vehicle-expense.read", vehicle_expense_tenant_id)
}

// ---------------------------------------------------------------------------------------------------
// Fuel
// ---------------------------------------------------------------------------------------------------

/// `EPIC-FU-01-S01` (`HRMS-942`, `C-029`): recording and curating fuellings -- the stores tier.
pub fn can_create_fuel_entry(actor: &User, target_tenant_id: Option<i64>) -> bool {
    ["fuel-entry.create", "fuel-entry.delete", "fuel-entry.restore", "fuel-entry.unify", "fuel-entry.sync-provider"]
        .iter()
        .any(|code| may_create_in(actor, code, target_tenant_id))
}

/// `EPIC-FU-03-S01` (`HRMS-948`, `TRM-520`): a driver reports their own fuelling from a receipt
/// (`fuel-entry.register-receipt@own`); the stores tier may report one on a driver's behalf.
pub fn can_report_fuel_receipt(actor: &User, target_tenant_id: Option<i64>) -> bool {
    may_create_in(actor, "fuel-entry.register-receipt", target_tenant_id)
}

pub fn can_read_fuel_entry(actor: &User, fuel_entry_tenant_id: Option<i64>) -> bool {
    may_act_on(actor, "fuel-entry.read", fuel_entry_tenant_id)
}

/// `EPIC-FU-07-S01` (`HRMS-943`, `C-029`): configuring the tank is a fleet-wide setting.
pub fn can_configure_internal_tank(actor: &User, target_tenant_id: Option<i64>) -> bool {
    may_create_in(actor, "internal-tank.update", target_tenant_id)
}

/// `EPIC-FU-06-S03` (`HRMS-953`): the gauge thresholds and the tenant's rule settings.
pub fn can_configure_fuel_gauge(actor: &User, target_tenant_id: Option<i64>) -> bool {
    ["fuel-gauge-settings.update", "tenant-rule-settings.update"]
        .iter()
        .any(|code| may_create_in(actor, code, target_tenant_id))
}

pub fn can_read_internal_tank(actor: &User, internal_tank_tenant_id: Option<i64>) -> bool {
    may_act_on_any(actor, &["internal-tank.read", "internal-tank.stock.read"], internal_tank_tenant_id)
}

// ---------------------------------------------------------------------------------------------------
// Garage
// ---------------------------------------------------------------------------------------------------

/// `EPIC-GA-01-S01` (`HRMS-956`, `C-028`): the garage catalogue.
pub fn can_manage_garage_catalogue(actor: &User, target_tenant_id: Option<i64>) -> bool {
    ["garage.service-catalogue.create", "garage.service-catalogue.update"]
        .iter()
        .any(|code| may_create_in(actor, code, target_tenant_id))
}

/// `EPIC-GA-02-S01` (`HRMS-958`): running the yard -- opening a triage, marking its services, checking a
/// vehicle out.
pub fn can_operate_garage(actor: &User, target_tenant_id: Option<i64>) -> bool {
    [
        "garage.triage.open",
        "garage.triage.mark-service",
        "garage.triage.check-out",
        "garage.service.internal",
        "garage.service.external",
    ]
    .iter()
    .any(|code| may_create_in(actor, code, target_tenant_id))
}

/// `EPIC-GA-03-S01` (`HRMS-960`, `TRM-788`): arrival and departure stamps are published by the platform's
/// own integration alone.
pub fn can_publish_presence(actor: &User) -> bool {
    is_platform_actor(actor) && actor.allows("vehicle-presence.publish")
}

/// `EPIC-GA-05-S02` (`HRMS-963`, `TRM-483`): calling a vehicle to base.
pub fn can_call_vehicle_to_base(actor: &User, target_tenant_id: Option<i64>) -> bool {
    ["garage.call.create", "garage.call.cancel"].iter().any(|code| may_create_in(actor, code, target_tenant_id))
}

pub fn can_read_garage(actor: &User, tenant_id: Option<i64>) -> bool {
    may_act_on_any(
        actor,
        &[
            "garage.triage.read",
            "garage.queue.read",
            "garage.monitor.read",
            "garage.call.read",
            "garage.call-list.read",
            "garage.service-catalogue.read",
        ],
        tenant_id,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::permission::{self, DRIVER, MECHANIC, SYS_ADMIN, TENANT_OWNER, TENANT_USER};
    use crate::domain::user::Access;

    fn actor(key: &str, tenant_id: Option<i64>) -> User {
        User {
            id: Some(1),
            uuid: None,
            email: "actor@example.com".to_string(),
            name: None,
            password: String::new(),
            enabled: true,
            tenant_id,
            profile_id: Some(1),
            extra_permissions: Vec::new(),
            cpf: None,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
            access: Access::of_system_profile(key).expect("a known system profile"),
        }
    }

    /// `C-036`: the root tenant's id in these tests; the platform actor belongs to it.
    const ROOT: i64 = 99;

    fn profile(administrator: bool, system: bool, tenant_id: Option<i64>) -> AccessProfile {
        AccessProfile {
            id: Some(9),
            uuid: Some("p".into()),
            tenant_id,
            name: "P".into(),
            name_key: "p".into(),
            system,
            key: administrator.then(|| SYS_ADMIN.to_string()),
            active: true,
            permissions: Vec::new(),
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
    }

    #[test]
    fn the_platform_actor_is_a_platform_profile_in_the_root_tenant_and_every_user_has_a_tenant() {
        assert!(is_platform_actor(&actor(SYS_ADMIN, Some(ROOT))));
        assert!(!is_platform_actor(&actor(TENANT_OWNER, Some(1))));
        assert!(lacks_required_tenant(&actor(SYS_ADMIN, None)));
        assert!(!lacks_required_tenant(&actor(SYS_ADMIN, Some(ROOT))));
        assert!(lacks_required_tenant(&actor(TENANT_OWNER, None)));
        assert!(!lacks_required_tenant(&actor(DRIVER, Some(1))));
    }

    #[test]
    fn a_capability_is_the_permission_inside_the_tenant() {
        let owner = actor(TENANT_OWNER, Some(1));
        let user = actor(TENANT_USER, Some(1));
        let platform = actor(SYS_ADMIN, Some(ROOT));
        assert!(can_administer_vehicle(&owner, Some(1)));
        assert!(!can_administer_vehicle(&owner, Some(2)));
        assert!(!can_administer_vehicle(&user, Some(1)));
        assert!(can_read_vehicle(&user, Some(1)));
        assert!(can_administer_vehicle(&platform, Some(2)));
        assert!(can_create_vehicle(&owner, None) && can_create_vehicle(&owner, Some(1)) && !can_create_vehicle(&owner, Some(2)));
        assert!(!can_create_vehicle(&platform, None) && can_create_vehicle(&platform, Some(5)));
    }

    #[test]
    fn an_extra_grant_widens_a_profile_and_a_missing_tenant_voids_it() {
        let mut user = actor(TENANT_USER, Some(1));
        assert!(!can_create_work_order(&user, Some(1)));
        user.access.grants.grant("work-order.create", permission::GrantScope::Full);
        assert!(can_create_work_order(&user, Some(1)));
        let homeless = actor(TENANT_OWNER, None);
        assert!(!can_create_vehicle(&homeless, None) && !can_read_cost_reports(&homeless));
    }

    #[test]
    fn the_driver_holds_own_scoped_grants_and_acts_as_a_driver() {
        let driver = actor(DRIVER, Some(1));
        assert!(driver.only_own("vehicle.read") && driver.only_own("checklist-run.create"));
        assert!(can_create_checklist_run(&driver, Some(1)) && !can_administer_checklist_run(&driver, Some(1)));
        assert!(is_active_driver_of(&driver, Some(1)) && !is_active_driver_of(&driver, Some(2)));
        assert!(!is_active_driver_of(&actor(MECHANIC, Some(1)), Some(1)));
        let mut off = actor(DRIVER, Some(1));
        off.enabled = false;
        assert!(!is_active_driver_of(&off, Some(1)));
    }

    #[test]
    fn the_platform_only_capabilities_need_both_the_scope_and_the_permission() {
        let platform = actor(SYS_ADMIN, Some(ROOT));
        let owner = actor(TENANT_OWNER, Some(1));
        assert!(can_create_tenant(&platform) && !can_create_tenant(&owner));
        assert!(can_manage_reference_data(&platform) && !can_manage_reference_data(&owner));
        assert!(can_publish_presence(&platform) && !can_publish_presence(&owner));
        assert!(can_link_tracker_device(&platform) && !can_link_tracker_device(&owner));
    }

    #[test]
    fn tenant_configuration_and_user_administration_follow_their_permissions() {
        let owner = actor(TENANT_OWNER, Some(1));
        let mechanic = actor(MECHANIC, Some(1));
        assert!(can_read_tenant_configuration(&owner) && !can_read_tenant_configuration(&mechanic));
        assert!(can_read_billing(&owner) && !can_read_billing(&mechanic));
        assert!(mechanic.allows("tenant.read"), "every tenant profile reads the tenant summary");
        assert!(can_update_tenant(&owner, 1) && !can_update_tenant(&owner, 2));
        assert!(can_administer_user(&owner, Some(2), Some(1)) && !can_administer_user(&owner, Some(2), Some(2)));
        assert!(!can_administer_user(&mechanic, Some(2), Some(1)));
        assert!(can_read_user_record(&mechanic, Some(1), Some(1)) && !can_read_user_record(&mechanic, Some(2), Some(1)));
        assert!(can_list_users(&owner) && !can_list_users(&mechanic));
        assert!(can_manage_access_profiles(&owner, Some(1)) && can_manage_access_profiles(&owner, None));
        assert!(!can_manage_access_profiles(&owner, Some(2)) && !can_manage_access_profiles(&mechanic, Some(1)));
    }

    #[test]
    fn placing_a_profile_follows_the_hierarchy() {
        let platform = actor(SYS_ADMIN, Some(ROOT));
        let owner = actor(TENANT_OWNER, Some(1));
        let platform_profile = profile(true, true, None);
        let system_tenant_profile = profile(false, true, None);
        let own_profile = profile(false, false, Some(1));
        let foreign_profile = profile(false, false, Some(2));
        assert_eq!(can_assign_profile(&platform, &platform_profile), Some(CreationTenant::Fixed(Some(ROOT))));
        assert_eq!(can_assign_profile(&platform, &own_profile), Some(CreationTenant::CallerChosen));
        assert_eq!(can_assign_profile(&owner, &platform_profile), None);
        assert_eq!(can_assign_profile(&owner, &system_tenant_profile), Some(CreationTenant::Fixed(Some(1))));
        assert_eq!(can_assign_profile(&owner, &own_profile), Some(CreationTenant::Fixed(Some(1))));
        assert_eq!(can_assign_profile(&owner, &foreign_profile), None);
        let mut inactive = own_profile.clone();
        inactive.active = false;
        assert_eq!(can_assign_profile(&owner, &inactive), None);
        assert!(profile_fits(&platform_profile, Some(ROOT)) && !profile_fits(&platform_profile, None));
        assert!(can_move_user_to_tenant(&platform, &platform_profile, Some(ROOT)) && !can_move_user_to_tenant(&platform, &platform_profile, Some(1)));
        assert!(profile_fits(&system_tenant_profile, Some(7)) && !profile_fits(&system_tenant_profile, None));
        assert!(profile_fits(&own_profile, Some(1)) && !profile_fits(&own_profile, Some(2)));
        assert!(can_move_user_to_tenant(&platform, &own_profile, Some(1)) && !can_move_user_to_tenant(&owner, &own_profile, Some(1)));
    }

    #[test]
    fn nobody_grants_what_they_do_not_hold() {
        let owner = actor(TENANT_OWNER, Some(1));
        let mechanic = actor(MECHANIC, Some(1));
        let driver_grants = Grants::from_codes(permission::system_grants(DRIVER));
        assert!(can_grant(&owner, &driver_grants));
        assert!(!can_grant(&mechanic, &Grants::from_codes(["user.create"])));
        assert!(!can_grant(&owner, &Grants::from_codes(["tenant.child.create"])));
        assert!(can_grant(&actor(SYS_ADMIN, Some(ROOT)), &Grants::from_codes(["tenant.child.create"])));
    }
}
