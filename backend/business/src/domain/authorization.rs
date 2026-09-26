//! EPIC-XF-02 (HRMS-012...017): the one shared authorization module AD-016
//! calls for. Before this, "a real SysAdmin has no tenant" (PD-013) was
//! written out verbatim in five places (`business_plan_endpoint::authorize`,
//! `tenant_endpoint::can_access_tenant` and its two inline call sites in
//! `add`/`add_plan`, and `authentication_middleware`'s `enforce_tenant` flag)
//! with nothing to stop them drifting apart.
//!
//! PD-020 bounds this deliberately: these are named **capabilities**, not a
//! permission/policy store. Call sites ask "can this actor do X", never "is
//! this actor role Y" -- so if a role-to-permission mapping is added later
//! (PD-020 anticipates but does not build it), only the bodies of these
//! functions change, not their call sites (HRMS-015).
//!
//! EPIC-IA-04 (HRMS-113...116, HRMS-118, PD-019) added the creation
//! hierarchy below: who may create a user of which role, and who may
//! reassign an existing one. `user_endpoint`'s current code is not an
//! unfinished version of that rule -- it implemented a *different* one
//! (every tenant-owner-created user forced to `TenantOwner`, and no path
//! could ever produce a `TenantUser` at all, U-007). Read PD-019's citation
//! in `02-system_requirements/hermes/identity-access_implementation_plan.md`
//! before changing anything below this point.
//!
//! `S05` (whether a tenant-bound administrator is even representable) is
//! left alone: it is an open decision (U-019, decision-register.md D-08),
//! not a refactor.

use crate::domain::enums::Role;
use crate::domain::user::User;

/// The one place PD-013's definition lives: an unbound platform
/// administrator is a `SysAdmin` with no tenant. Every other capability in
/// this module is built on it (or, per PD-020, could be replaced
/// independently of it later).
pub fn is_unbound_sys_admin(user: &User) -> bool {
    user.role == Role::SysAdmin && user.tenant_id.is_none()
}

/// HRMS-111/HRMS-133: every role but the platform administrator is
/// tenant-bound, the operational roles included (EPIC-IA-09-S05). An account
/// that breaks that is refused outright rather than treated as unscoped.
pub fn lacks_required_tenant(user: &User) -> bool {
    user.role != Role::SysAdmin && user.tenant_id.is_none()
}

/// Whether `user` may read or act on the given tenant's own data: an
/// unbound platform administrator (any tenant), or a member of that exact
/// tenant (HRM-090...096).
pub fn can_access_tenant(user: &User, tenant_id: i64) -> bool {
    user.tenant_id == Some(tenant_id) || is_unbound_sys_admin(user)
}

/// `POST /tenant` -- only an unbound platform administrator may create a
/// tenant (PD-019).
pub fn can_create_tenant(user: &User) -> bool {
    is_unbound_sys_admin(user)
}

/// The business-plan catalogue is platform-global (PD-014); only an
/// unbound platform administrator may read or write it.
pub fn can_manage_business_plan_catalogue(user: &User) -> bool {
    is_unbound_sys_admin(user)
}

/// PD-027: reference data (provinces, cities) is platform-global, exactly like
/// the business-plan catalogue, so the same rule governs it -- only an unbound
/// platform administrator may write it. A tenant editing the country's city
/// list would be editing every other tenant's address options.
pub fn can_manage_reference_data(user: &User) -> bool {
    is_unbound_sys_admin(user)
}

/// `POST /tenant/{id}/plan` -- only an unbound platform administrator may
/// set a tenant's plan (HRMS-224, PD-019, PD-021).
pub fn can_set_tenant_plan(user: &User) -> bool {
    is_unbound_sys_admin(user)
}

/// Where a newly created user's tenant comes from, once
/// [`can_create_user_with_role`] has decided the creation is allowed at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreationTenant {
    /// Fixed by the hierarchy, not the caller's choice: `None` for a new
    /// platform administrator, `Some(id)` for a tenant owner creating a
    /// tenant user in their own tenant (HRMS-115, HRMS-118).
    Fixed(Option<i64>),
    /// The caller names the tenant (a platform administrator creating a
    /// tenant owner, HRMS-114). The caller of this function is responsible
    /// for validating that the named tenant actually exists -- this module
    /// has no I/O and cannot do that itself.
    CallerChosen,
}

/// PD-019's creation hierarchy (EPIC-IA-04): an unbound platform
/// administrator creates platform administrators and tenant owners; a
/// tenant owner creates tenant users, drivers and mechanics in their own
/// tenant only (EPIC-IA-09-S03, HRMS-131, D-22). Every
/// other combination -- including a platform administrator creating a
/// tenant user directly, which is the tenant owner's job -- is rejected
/// (`None`), per S05's "every combination outside the hierarchy is
/// rejected."
pub fn can_create_user_with_role(actor: &User, target_role: &Role) -> Option<CreationTenant> {
    if is_unbound_sys_admin(actor) {
        match target_role {
            Role::SysAdmin => Some(CreationTenant::Fixed(None)),
            Role::TenantOwner => Some(CreationTenant::CallerChosen),
            Role::TenantUser | Role::Driver | Role::Mechanic => None,
        }
    } else if actor.role == Role::TenantOwner {
        match target_role {
            Role::TenantUser | Role::Driver | Role::Mechanic => {
                actor.tenant_id.map(|id| CreationTenant::Fixed(Some(id)))
            }
            _ => None,
        }
    } else {
        None
    }
}

/// Whether `actor` may set an *existing* user's role and tenant to exactly
/// this combination while editing them (PD-019). Reassigning the hierarchy
/// is an unbound platform administrator's decision alone, and even then
/// never assigns `TenantUser` -- that role is only ever created by a
/// tenant owner ([`can_create_user_with_role`]), never reassigned onto an
/// existing account by an edit. When this returns `false`, the caller
/// keeps the account's existing role and tenant rather than reject the
/// whole edit -- that is what lets a platform administrator (or the
/// account's own owner) update an existing tenant user's other fields
/// without being forced to also re-decide their place in the hierarchy.
pub fn can_reassign_role(
    actor: &User,
    requested_role: &Role,
    requested_tenant_id: Option<i64>,
) -> bool {
    is_unbound_sys_admin(actor)
        && match requested_role {
            Role::SysAdmin => requested_tenant_id.is_none(),
            Role::TenantOwner => requested_tenant_id.is_some(),
            // Like `TenantUser`, the operational roles are only ever
            // created by a tenant owner (D-22), never reassigned onto an
            // existing account by an edit.
            Role::TenantUser | Role::Driver | Role::Mechanic => false,
        }
}

/// Whether `actor` may *read* `target`'s record (EPIC-IA-05-S01): the target
/// themselves, an unbound platform administrator (any target), or a tenant
/// owner whose tenant matches the target's exactly.
///
/// Reading your own record is self-service, not administration -- see
/// [`can_administer_user`] for the write side, which is a strictly smaller
/// set.
pub fn can_read_user_record(
    actor: &User,
    target_id: Option<i64>,
    target_tenant_id: Option<i64>,
) -> bool {
    (actor.id.is_some() && actor.id == target_id)
        || can_administer_user(actor, target_id, target_tenant_id)
}

/// Whether `actor` may administer `target`'s record -- edit it, not read it
/// (EPIC-IA-05-S01, HRMS-117, HRMS-118): an unbound platform administrator
/// (any target), or a tenant owner whose tenant matches the target's exactly.
///
/// DEF-IA-03: "the target themselves" used to be an arm of this rule, which
/// made `PUT /user/{own id}` a user-administration path open to every role --
/// HRMS-117 says a tenant user has no user-administration rights at all, and
/// their own record is not an exception to that. What a user may do to their
/// own account without administering it (change their password, knowing the
/// current one) has its own route.
pub fn can_administer_user(
    actor: &User,
    _target_id: Option<i64>,
    target_tenant_id: Option<i64>,
) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.role == Role::TenantOwner && actor.tenant_id == target_tenant_id)
}

/// EPIC-FO-01-S04 (HRMS-923, PD-019): who may register, edit or retire a
/// vehicle. Deliberately *not* a new hierarchy -- it is the one the platform
/// already enforces for users, read one level down: an unbound platform
/// administrator acts across tenants, a tenant owner administers their own
/// tenant, and a `TenantUser` administers nothing (HRMS-117's rule, applied
/// to fleet records rather than to accounts).
///
/// PD-026's Driver and Mechanic roles do not exist yet (`Role` has three
/// variants), so nothing here anticipates them: when they arrive they change
/// the body of this function and not its call sites, which is the whole point
/// of PD-020.
pub fn can_administer_vehicle(actor: &User, vehicle_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.role == Role::TenantOwner
            && actor.tenant_id.is_some()
            && actor.tenant_id == vehicle_tenant_id)
}

/// `POST /vehicle` -- the create side of [`can_administer_vehicle`], asked
/// before any vehicle exists to compare tenants against. An unbound platform
/// administrator names the owning tenant; a tenant owner may only ever create
/// inside their own, so one without a tenant creates nothing.
pub fn can_create_vehicle(actor: &User, target_tenant_id: Option<i64>) -> bool {
    if is_unbound_sys_admin(actor) {
        return target_tenant_id.is_some();
    }
    actor.role == Role::TenantOwner
        && actor.tenant_id.is_some()
        && (target_tenant_id.is_none() || target_tenant_id == actor.tenant_id)
}

/// EPIC-FO-01-S04/S05 (HRMS-923, HRMS-924): reading a vehicle is wider than
/// administering it -- a `TenantUser` is a member of the fleet's tenant and
/// needs to see it (a driver looking up their own vehicle is this case), they
/// simply may not change it. Cross-tenant is not a narrower permission but no
/// visibility at all, which is why `vehicle_endpoint` answers 404 there and
/// not 403 (PD-034).
pub fn can_read_vehicle(actor: &User, vehicle_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.tenant_id.is_some() && actor.tenant_id == vehicle_tenant_id)
}

/// EPIC-FO-02-S01/S02 (HRMS-926, HRMS-927): linking a tracking device is
/// the platform administrator's act. The provider account is the platform's,
/// so a tenant that could name any device id could read another customer's
/// vehicle position through its own vehicle record.
pub fn can_link_tracker_device(actor: &User) -> bool {
    is_unbound_sys_admin(actor)
}

/// `EPIC-SC-01-S01` (`HRMS-600`, `C-024`): who may administer a customer
/// record -- the same hierarchy `can_administer_vehicle` already enforces,
/// applied to another tenant-owned registry rather than to fleet records.
/// Named rather than reused (`PD-020`): a role-to-permission mapping added
/// later changes this function's body, not `customer_endpoint`'s call site.
pub fn can_administer_customer(actor: &User, customer_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.role == Role::TenantOwner
            && actor.tenant_id.is_some()
            && actor.tenant_id == customer_tenant_id)
}

/// `POST /customer` -- the create side of [`can_administer_customer`], asked
/// before any customer exists to compare tenants against.
pub fn can_create_customer(actor: &User, target_tenant_id: Option<i64>) -> bool {
    if is_unbound_sys_admin(actor) {
        return target_tenant_id.is_some();
    }
    actor.role == Role::TenantOwner
        && actor.tenant_id.is_some()
        && (target_tenant_id.is_none() || target_tenant_id == actor.tenant_id)
}

/// Reading a customer is wider than administering one -- every role in the
/// owning tenant may see it, the same relationship `can_read_vehicle` has to
/// `can_administer_vehicle`.
pub fn can_read_customer(actor: &User, customer_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.tenant_id.is_some() && actor.tenant_id == customer_tenant_id)
}

/// `EPIC-SC-01-S03` (`HRMS-602`, `C-024`): the same hierarchy every other
/// tenant-owned registry uses. Named rather than reused (`PD-020`).
pub fn can_administer_holiday(actor: &User, holiday_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.role == Role::TenantOwner
            && actor.tenant_id.is_some()
            && actor.tenant_id == holiday_tenant_id)
}

pub fn can_create_holiday(actor: &User, target_tenant_id: Option<i64>) -> bool {
    if is_unbound_sys_admin(actor) {
        return target_tenant_id.is_some();
    }
    actor.role == Role::TenantOwner
        && actor.tenant_id.is_some()
        && (target_tenant_id.is_none() || target_tenant_id == actor.tenant_id)
}

pub fn can_read_holiday(actor: &User, holiday_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.tenant_id.is_some() && actor.tenant_id == holiday_tenant_id)
}

/// `EPIC-SC-02-S01` (`HRMS-603`, `C-024`): the same hierarchy every other
/// tenant-owned registry uses. Named rather than reused (`PD-020`).
pub fn can_administer_transport_demand(actor: &User, demand_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.role == Role::TenantOwner
            && actor.tenant_id.is_some()
            && actor.tenant_id == demand_tenant_id)
}

pub fn can_create_transport_demand(actor: &User, target_tenant_id: Option<i64>) -> bool {
    if is_unbound_sys_admin(actor) {
        return target_tenant_id.is_some();
    }
    actor.role == Role::TenantOwner
        && actor.tenant_id.is_some()
        && (target_tenant_id.is_none() || target_tenant_id == actor.tenant_id)
}

pub fn can_read_transport_demand(actor: &User, demand_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.tenant_id.is_some() && actor.tenant_id == demand_tenant_id)
}

/// `EPIC-SC-03-S01` (`HRMS-607`, `C-024`): the same hierarchy every other
/// tenant-owned registry uses. Named rather than reused (`PD-020`).
pub fn can_administer_extra_trip(actor: &User, trip_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.role == Role::TenantOwner
            && actor.tenant_id.is_some()
            && actor.tenant_id == trip_tenant_id)
}

pub fn can_create_extra_trip(actor: &User, target_tenant_id: Option<i64>) -> bool {
    if is_unbound_sys_admin(actor) {
        return target_tenant_id.is_some();
    }
    actor.role == Role::TenantOwner
        && actor.tenant_id.is_some()
        && (target_tenant_id.is_none() || target_tenant_id == actor.tenant_id)
}

pub fn can_read_extra_trip(actor: &User, trip_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.tenant_id.is_some() && actor.tenant_id == trip_tenant_id)
}

/// `EPIC-CK-02-S01` (`HRMS-651`, `C-025`): the same hierarchy every other
/// tenant-owned registry uses. Named rather than reused (`PD-020`).
pub fn can_administer_checklist_template(actor: &User, template_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.role == Role::TenantOwner
            && actor.tenant_id.is_some()
            && actor.tenant_id == template_tenant_id)
}

pub fn can_create_checklist_template(actor: &User, target_tenant_id: Option<i64>) -> bool {
    if is_unbound_sys_admin(actor) {
        return target_tenant_id.is_some();
    }
    actor.role == Role::TenantOwner
        && actor.tenant_id.is_some()
        && (target_tenant_id.is_none() || target_tenant_id == actor.tenant_id)
}

pub fn can_read_checklist_template(actor: &User, template_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.tenant_id.is_some() && actor.tenant_id == template_tenant_id)
}

/// `EPIC-CK-03-S01` (`HRMS-652`, `C-025`): the same hierarchy every other
/// tenant-owned registry uses. Named rather than reused (`PD-020`). No
/// self-service Driver create yet -- `C-031` (driver mobile app) is the
/// story that gives a `Driver` caller a client to submit their own
/// checklist from; until then, submission is administrative, the same
/// shape `extra_trip`'s `driver_id` already lets an owner name any driver.
pub fn can_administer_checklist_run(actor: &User, run_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.role == Role::TenantOwner
            && actor.tenant_id.is_some()
            && actor.tenant_id == run_tenant_id)
}

pub fn can_create_checklist_run(actor: &User, target_tenant_id: Option<i64>) -> bool {
    if is_unbound_sys_admin(actor) {
        return target_tenant_id.is_some();
    }
    actor.role == Role::TenantOwner
        && actor.tenant_id.is_some()
        && (target_tenant_id.is_none() || target_tenant_id == actor.tenant_id)
}

pub fn can_read_checklist_run(actor: &User, run_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.tenant_id.is_some() && actor.tenant_id == run_tenant_id)
}

/// `EPIC-MT-01-S01` (`HRMS-700`, `C-026`): the same hierarchy every other
/// tenant-owned registry uses, widened to `Mechanic` for create -- the
/// accepted story names "tenant owner or mechanic," and `Role::Mechanic` is
/// already scoped to "garage and maintenance only" (`EPIC-IA-09`), unlike
/// `Driver`'s own create-side deferral elsewhere in this program, which
/// waits on a client that does not exist yet. Named rather than reused
/// (`PD-020`).
pub fn can_administer_work_order(actor: &User, work_order_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.role == Role::TenantOwner
            && actor.tenant_id.is_some()
            && actor.tenant_id == work_order_tenant_id)
}

pub fn can_create_work_order(actor: &User, target_tenant_id: Option<i64>) -> bool {
    if is_unbound_sys_admin(actor) {
        return target_tenant_id.is_some();
    }
    matches!(actor.role, Role::TenantOwner | Role::Mechanic)
        && actor.tenant_id.is_some()
        && (target_tenant_id.is_none() || target_tenant_id == actor.tenant_id)
}

pub fn can_read_work_order(actor: &User, work_order_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.tenant_id.is_some() && actor.tenant_id == work_order_tenant_id)
}

/// `EPIC-MT-03-S01` (`HRMS-703`, `C-026`): the same hierarchy `work_order`
/// uses, widened to `Mechanic` for create for the same reason (`TRM-231`
/// names "the manager," but scheduling a garage visit is squarely
/// maintenance work, not an owner-only act). Named rather than reused
/// (`PD-020`).
pub fn can_administer_maintenance_plan(actor: &User, plan_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.role == Role::TenantOwner
            && actor.tenant_id.is_some()
            && actor.tenant_id == plan_tenant_id)
}

pub fn can_create_maintenance_plan(actor: &User, target_tenant_id: Option<i64>) -> bool {
    if is_unbound_sys_admin(actor) {
        return target_tenant_id.is_some();
    }
    matches!(actor.role, Role::TenantOwner | Role::Mechanic)
        && actor.tenant_id.is_some()
        && (target_tenant_id.is_none() || target_tenant_id == actor.tenant_id)
}

pub fn can_read_maintenance_plan(actor: &User, plan_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.tenant_id.is_some() && actor.tenant_id == plan_tenant_id)
}

/// `EPIC-MT-05-S01` (`HRMS-704`, `C-026`): the same hierarchy every other
/// tenant-owned registry uses -- `TenantOwner`/`SysAdmin` only, unlike
/// `work_order`/`maintenance_plan`. Classifying service categories is a
/// configuration task, not day-to-day maintenance work a `Mechanic` does.
/// Named rather than reused (`PD-020`).
pub fn can_administer_service_type(actor: &User, service_type_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.role == Role::TenantOwner
            && actor.tenant_id.is_some()
            && actor.tenant_id == service_type_tenant_id)
}

pub fn can_create_service_type(actor: &User, target_tenant_id: Option<i64>) -> bool {
    if is_unbound_sys_admin(actor) {
        return target_tenant_id.is_some();
    }
    actor.role == Role::TenantOwner
        && actor.tenant_id.is_some()
        && (target_tenant_id.is_none() || target_tenant_id == actor.tenant_id)
}

pub fn can_read_service_type(actor: &User, service_type_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.tenant_id.is_some() && actor.tenant_id == service_type_tenant_id)
}

/// Same shape as `service_type` -- a sibling catalogue, same authorization.
pub fn can_administer_priced_service(actor: &User, priced_service_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.role == Role::TenantOwner
            && actor.tenant_id.is_some()
            && actor.tenant_id == priced_service_tenant_id)
}

pub fn can_create_priced_service(actor: &User, target_tenant_id: Option<i64>) -> bool {
    if is_unbound_sys_admin(actor) {
        return target_tenant_id.is_some();
    }
    actor.role == Role::TenantOwner
        && actor.tenant_id.is_some()
        && (target_tenant_id.is_none() || target_tenant_id == actor.tenant_id)
}

pub fn can_read_priced_service(actor: &User, priced_service_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        || (actor.tenant_id.is_some() && actor.tenant_id == priced_service_tenant_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(role: Role, tenant_id: Option<i64>) -> User {
        User {
            id: Some(1),
            uuid: None,
            email: "actor@example.com".to_string(),
            name: None,
            password: String::new(),
            enabled: true,
            tenant_id,
            role,
            created_at: None,
            created_by: None,
            updated_at: None,
            updated_by: None,
        }
    }

    /// HRMS-017 (EPIC-XF-02-S06): a role x operation matrix. Every
    /// PD-013-shaped capability in this module reduces to `is_unbound_sys_admin`,
    /// so proving it once here and asserting the others delegate is the
    /// matrix -- not restating the same three cases four more times.
    #[test]
    fn is_unbound_sys_admin_matrix() {
        assert!(is_unbound_sys_admin(&user(Role::SysAdmin, None)));
        assert!(!is_unbound_sys_admin(&user(Role::SysAdmin, Some(1))));
        assert!(!is_unbound_sys_admin(&user(Role::TenantOwner, None)));
        assert!(!is_unbound_sys_admin(&user(Role::TenantOwner, Some(1))));
        assert!(!is_unbound_sys_admin(&user(Role::TenantUser, Some(1))));
    }

    #[test]
    fn only_the_unbound_platform_administrator_links_a_tracker_device() {
        assert!(can_link_tracker_device(&user(Role::SysAdmin, None)));
        assert!(!can_link_tracker_device(&user(Role::SysAdmin, Some(1))));
        assert!(!can_link_tracker_device(&user(Role::TenantOwner, Some(1))));
        assert!(!can_link_tracker_device(&user(Role::TenantUser, Some(1))));
    }

    #[test]
    fn customer_authorization_matches_the_vehicle_shape() {
        let sysadmin_unbound = user(Role::SysAdmin, None);
        let sysadmin_bound = user(Role::SysAdmin, Some(1));
        let owner_1 = user(Role::TenantOwner, Some(1));
        let owner_2 = user(Role::TenantOwner, Some(2));
        let member_1 = user(Role::TenantUser, Some(1));

        assert!(can_create_customer(&sysadmin_unbound, Some(1)));
        assert!(!can_create_customer(&sysadmin_bound, Some(1)));
        assert!(can_create_customer(&owner_1, Some(1)));
        assert!(!can_create_customer(&owner_1, Some(2)));
        assert!(!can_create_customer(&member_1, Some(1)));

        assert!(can_administer_customer(&sysadmin_unbound, Some(1)));
        assert!(can_administer_customer(&owner_1, Some(1)));
        assert!(!can_administer_customer(&owner_2, Some(1)));
        assert!(!can_administer_customer(&member_1, Some(1)));

        assert!(can_read_customer(&sysadmin_unbound, Some(1)));
        assert!(can_read_customer(&member_1, Some(1)));
        assert!(!can_read_customer(&member_1, Some(2)));
    }

    /// `EPIC-SC-01-S03`: identical shape to `customer_authorization_matches_
    /// the_vehicle_shape` -- proven once there, checked here for real rather
    /// than assumed, since these are independently-declared functions.
    #[test]
    fn holiday_authorization_matches_the_customer_shape() {
        let sysadmin_unbound = user(Role::SysAdmin, None);
        let owner_1 = user(Role::TenantOwner, Some(1));
        let owner_2 = user(Role::TenantOwner, Some(2));
        let member_1 = user(Role::TenantUser, Some(1));

        assert!(can_create_holiday(&sysadmin_unbound, Some(1)));
        assert!(can_create_holiday(&owner_1, Some(1)));
        assert!(!can_create_holiday(&owner_1, Some(2)));
        assert!(!can_create_holiday(&member_1, Some(1)));

        assert!(can_administer_holiday(&owner_1, Some(1)));
        assert!(!can_administer_holiday(&owner_2, Some(1)));

        assert!(can_read_holiday(&member_1, Some(1)));
        assert!(!can_read_holiday(&member_1, Some(2)));
    }

    #[test]
    fn transport_demand_authorization_matches_the_customer_shape() {
        let sysadmin_unbound = user(Role::SysAdmin, None);
        let owner_1 = user(Role::TenantOwner, Some(1));
        let owner_2 = user(Role::TenantOwner, Some(2));
        let member_1 = user(Role::TenantUser, Some(1));

        assert!(can_create_transport_demand(&sysadmin_unbound, Some(1)));
        assert!(can_create_transport_demand(&owner_1, Some(1)));
        assert!(!can_create_transport_demand(&owner_1, Some(2)));
        assert!(!can_create_transport_demand(&member_1, Some(1)));

        assert!(can_administer_transport_demand(&owner_1, Some(1)));
        assert!(!can_administer_transport_demand(&owner_2, Some(1)));

        assert!(can_read_transport_demand(&member_1, Some(1)));
        assert!(!can_read_transport_demand(&member_1, Some(2)));
    }

    #[test]
    fn extra_trip_authorization_matches_the_customer_shape() {
        let sysadmin_unbound = user(Role::SysAdmin, None);
        let owner_1 = user(Role::TenantOwner, Some(1));
        let owner_2 = user(Role::TenantOwner, Some(2));
        let member_1 = user(Role::TenantUser, Some(1));

        assert!(can_create_extra_trip(&sysadmin_unbound, Some(1)));
        assert!(can_create_extra_trip(&owner_1, Some(1)));
        assert!(!can_create_extra_trip(&owner_1, Some(2)));
        assert!(!can_create_extra_trip(&member_1, Some(1)));

        assert!(can_administer_extra_trip(&owner_1, Some(1)));
        assert!(!can_administer_extra_trip(&owner_2, Some(1)));

        assert!(can_read_extra_trip(&member_1, Some(1)));
        assert!(!can_read_extra_trip(&member_1, Some(2)));
    }

    #[test]
    fn checklist_template_authorization_matches_the_customer_shape() {
        let sysadmin_unbound = user(Role::SysAdmin, None);
        let owner_1 = user(Role::TenantOwner, Some(1));
        let owner_2 = user(Role::TenantOwner, Some(2));
        let member_1 = user(Role::TenantUser, Some(1));

        assert!(can_create_checklist_template(&sysadmin_unbound, Some(1)));
        assert!(can_create_checklist_template(&owner_1, Some(1)));
        assert!(!can_create_checklist_template(&owner_1, Some(2)));
        assert!(!can_create_checklist_template(&member_1, Some(1)));

        assert!(can_administer_checklist_template(&owner_1, Some(1)));
        assert!(!can_administer_checklist_template(&owner_2, Some(1)));

        assert!(can_read_checklist_template(&member_1, Some(1)));
        assert!(!can_read_checklist_template(&member_1, Some(2)));
    }

    #[test]
    fn checklist_run_authorization_matches_the_customer_shape() {
        let sysadmin_unbound = user(Role::SysAdmin, None);
        let owner_1 = user(Role::TenantOwner, Some(1));
        let owner_2 = user(Role::TenantOwner, Some(2));
        let member_1 = user(Role::TenantUser, Some(1));

        assert!(can_create_checklist_run(&sysadmin_unbound, Some(1)));
        assert!(can_create_checklist_run(&owner_1, Some(1)));
        assert!(!can_create_checklist_run(&owner_1, Some(2)));
        assert!(!can_create_checklist_run(&member_1, Some(1)));

        assert!(can_administer_checklist_run(&owner_1, Some(1)));
        assert!(!can_administer_checklist_run(&owner_2, Some(1)));

        assert!(can_read_checklist_run(&member_1, Some(1)));
        assert!(!can_read_checklist_run(&member_1, Some(2)));
    }

    /// `EPIC-MT-01-S01`: the one place this shape differs from its
    /// siblings -- `Mechanic` may create, the same as `TenantOwner`.
    #[test]
    fn work_order_authorization_widens_create_to_the_mechanic_role() {
        let sysadmin_unbound = user(Role::SysAdmin, None);
        let owner_1 = user(Role::TenantOwner, Some(1));
        let owner_2 = user(Role::TenantOwner, Some(2));
        let mechanic_1 = user(Role::Mechanic, Some(1));
        let member_1 = user(Role::TenantUser, Some(1));

        assert!(can_create_work_order(&sysadmin_unbound, Some(1)));
        assert!(can_create_work_order(&owner_1, Some(1)));
        assert!(can_create_work_order(&mechanic_1, Some(1)));
        assert!(!can_create_work_order(&mechanic_1, Some(2)));
        assert!(!can_create_work_order(&member_1, Some(1)));

        assert!(can_administer_work_order(&owner_1, Some(1)));
        assert!(!can_administer_work_order(&owner_2, Some(1)));
        assert!(
            !can_administer_work_order(&mechanic_1, Some(1)),
            "administer stays tenant-owner-only until an update/conclude story exists"
        );

        assert!(can_read_work_order(&member_1, Some(1)));
        assert!(!can_read_work_order(&member_1, Some(2)));
    }

    #[test]
    fn maintenance_plan_authorization_widens_create_to_the_mechanic_role() {
        let sysadmin_unbound = user(Role::SysAdmin, None);
        let owner_1 = user(Role::TenantOwner, Some(1));
        let owner_2 = user(Role::TenantOwner, Some(2));
        let mechanic_1 = user(Role::Mechanic, Some(1));
        let member_1 = user(Role::TenantUser, Some(1));

        assert!(can_create_maintenance_plan(&sysadmin_unbound, Some(1)));
        assert!(can_create_maintenance_plan(&owner_1, Some(1)));
        assert!(can_create_maintenance_plan(&mechanic_1, Some(1)));
        assert!(!can_create_maintenance_plan(&mechanic_1, Some(2)));
        assert!(!can_create_maintenance_plan(&member_1, Some(1)));

        assert!(can_administer_maintenance_plan(&owner_1, Some(1)));
        assert!(!can_administer_maintenance_plan(&owner_2, Some(1)));

        assert!(can_read_maintenance_plan(&member_1, Some(1)));
        assert!(!can_read_maintenance_plan(&member_1, Some(2)));
    }

    #[test]
    fn service_type_authorization_matches_the_customer_shape() {
        let sysadmin_unbound = user(Role::SysAdmin, None);
        let owner_1 = user(Role::TenantOwner, Some(1));
        let owner_2 = user(Role::TenantOwner, Some(2));
        let mechanic_1 = user(Role::Mechanic, Some(1));
        let member_1 = user(Role::TenantUser, Some(1));

        assert!(can_create_service_type(&sysadmin_unbound, Some(1)));
        assert!(can_create_service_type(&owner_1, Some(1)));
        assert!(!can_create_service_type(&owner_1, Some(2)));
        assert!(
            !can_create_service_type(&mechanic_1, Some(1)),
            "unlike work_order/maintenance_plan, a Mechanic may not create service types"
        );
        assert!(!can_create_service_type(&member_1, Some(1)));

        assert!(can_administer_service_type(&owner_1, Some(1)));
        assert!(!can_administer_service_type(&owner_2, Some(1)));

        assert!(can_read_service_type(&member_1, Some(1)));
        assert!(!can_read_service_type(&member_1, Some(2)));
    }

    #[test]
    fn priced_service_authorization_matches_the_customer_shape() {
        let sysadmin_unbound = user(Role::SysAdmin, None);
        let owner_1 = user(Role::TenantOwner, Some(1));
        let owner_2 = user(Role::TenantOwner, Some(2));
        let member_1 = user(Role::TenantUser, Some(1));

        assert!(can_create_priced_service(&sysadmin_unbound, Some(1)));
        assert!(can_create_priced_service(&owner_1, Some(1)));
        assert!(!can_create_priced_service(&owner_1, Some(2)));
        assert!(!can_create_priced_service(&member_1, Some(1)));

        assert!(can_administer_priced_service(&owner_1, Some(1)));
        assert!(!can_administer_priced_service(&owner_2, Some(1)));

        assert!(can_read_priced_service(&member_1, Some(1)));
        assert!(!can_read_priced_service(&member_1, Some(2)));
    }

    #[test]
    fn can_access_tenant_matrix() {
        let sysadmin = user(Role::SysAdmin, None);
        assert!(can_access_tenant(&sysadmin, 1));
        assert!(can_access_tenant(&sysadmin, 999));

        let owner_of_1 = user(Role::TenantOwner, Some(1));
        assert!(can_access_tenant(&owner_of_1, 1));
        assert!(!can_access_tenant(&owner_of_1, 2));

        let member_of_1 = user(Role::TenantUser, Some(1));
        assert!(can_access_tenant(&member_of_1, 1));
        assert!(!can_access_tenant(&member_of_1, 2));

        // A tenant-bound SysAdmin (U-019: whether this is even a
        // representable state is still open) is not treated as unbound --
        // it falls back to the plain tenant-match rule, same as any member.
        let bound_sysadmin = user(Role::SysAdmin, Some(1));
        assert!(can_access_tenant(&bound_sysadmin, 1));
        assert!(!can_access_tenant(&bound_sysadmin, 2));
    }

    #[test]
    fn create_tenant_manage_catalogue_and_set_plan_agree_with_is_unbound_sys_admin() {
        for (role, tenant_id) in [
            (Role::SysAdmin, None),
            (Role::SysAdmin, Some(1)),
            (Role::TenantOwner, Some(1)),
            (Role::TenantUser, Some(1)),
        ] {
            let actor = user(role, tenant_id);
            let expected = is_unbound_sys_admin(&actor);
            assert_eq!(can_create_tenant(&actor), expected);
            assert_eq!(can_manage_business_plan_catalogue(&actor), expected);
            assert_eq!(can_set_tenant_plan(&actor), expected);
            assert_eq!(can_manage_reference_data(&actor), expected);
        }
    }

    /// HRMS-113...116 (EPIC-IA-04-S01...S05): the full role x target-role
    /// creation matrix. Every combination is asserted, not just the
    /// permitted ones -- S05 explicitly wants "outside the hierarchy" to be
    /// a rejection, and the platform-admin-creates-tenant-user cell is the
    /// one the pre-fix code got backwards (it forced the opposite).
    #[test]
    fn can_create_user_with_role_matrix() {
        let sysadmin = user(Role::SysAdmin, None);
        assert_eq!(
            can_create_user_with_role(&sysadmin, &Role::SysAdmin),
            Some(CreationTenant::Fixed(None))
        );
        assert_eq!(
            can_create_user_with_role(&sysadmin, &Role::TenantOwner),
            Some(CreationTenant::CallerChosen)
        );
        assert_eq!(
            can_create_user_with_role(&sysadmin, &Role::TenantUser),
            None
        );

        let owner = user(Role::TenantOwner, Some(7));
        assert_eq!(can_create_user_with_role(&owner, &Role::SysAdmin), None);
        assert_eq!(can_create_user_with_role(&owner, &Role::TenantOwner), None);
        assert_eq!(
            can_create_user_with_role(&owner, &Role::TenantUser),
            Some(CreationTenant::Fixed(Some(7)))
        );

        let member = user(Role::TenantUser, Some(7));
        for target in [Role::SysAdmin, Role::TenantOwner, Role::TenantUser] {
            assert_eq!(can_create_user_with_role(&member, &target), None);
        }

        // A tenant-bound SysAdmin is not unbound (U-019 is still open on
        // whether this state should even exist) -- today it creates nothing.
        let bound_sysadmin = user(Role::SysAdmin, Some(1));
        for target in [Role::SysAdmin, Role::TenantOwner, Role::TenantUser] {
            assert_eq!(can_create_user_with_role(&bound_sysadmin, &target), None);
        }
    }

    #[test]
    fn can_reassign_role_only_an_unbound_sys_admin_and_never_to_tenant_user() {
        let sysadmin = user(Role::SysAdmin, None);
        assert!(can_reassign_role(&sysadmin, &Role::SysAdmin, None));
        assert!(!can_reassign_role(&sysadmin, &Role::SysAdmin, Some(1)));
        assert!(can_reassign_role(&sysadmin, &Role::TenantOwner, Some(1)));
        assert!(!can_reassign_role(&sysadmin, &Role::TenantOwner, None));
        assert!(!can_reassign_role(&sysadmin, &Role::TenantUser, Some(1)));

        let owner = user(Role::TenantOwner, Some(1));
        assert!(!can_reassign_role(&owner, &Role::TenantOwner, Some(1)));
        assert!(!can_reassign_role(&owner, &Role::TenantUser, Some(1)));
    }

    /// EPIC-IA-09-S02/S03/S05 (HRMS-130, HRMS-131, HRMS-133, D-22): a tenant
    /// owner creates drivers and mechanics in their own tenant; nobody else
    /// does, no edit reassigns onto them, and neither role administers
    /// anything that exists today. They read their own tenant's vehicles.
    #[test]
    fn operational_roles_matrix() {
        let owner = user(Role::TenantOwner, Some(7));
        let sysadmin = user(Role::SysAdmin, None);
        for role in [Role::Driver, Role::Mechanic] {
            assert_eq!(
                can_create_user_with_role(&owner, &role),
                Some(CreationTenant::Fixed(Some(7)))
            );
            assert_eq!(can_create_user_with_role(&sysadmin, &role), None);
            assert!(!can_reassign_role(&sysadmin, &role, Some(7)));

            let actor = user(role.clone(), Some(7));
            for target in [Role::SysAdmin, Role::TenantOwner, Role::TenantUser, Role::Driver, Role::Mechanic] {
                assert_eq!(can_create_user_with_role(&actor, &target), None);
            }
            assert!(!is_unbound_sys_admin(&actor));
            assert!(!can_create_tenant(&actor));
            assert!(!can_set_tenant_plan(&actor));
            assert!(!can_manage_business_plan_catalogue(&actor));
            assert!(!can_manage_reference_data(&actor));
            assert!(!can_administer_user(&actor, Some(99), Some(7)));
            assert!(!can_create_vehicle(&actor, Some(7)));
            assert!(!can_administer_vehicle(&actor, Some(7)));
            assert!(!can_link_tracker_device(&actor));
            assert!(can_read_vehicle(&actor, Some(7)));
            assert!(!can_read_vehicle(&actor, Some(8)));
            assert!(lacks_required_tenant(&user(role.clone(), None)));
            assert!(!lacks_required_tenant(&actor));
        }
        assert!(!lacks_required_tenant(&sysadmin));
    }

    #[test]
    fn can_administer_user_matrix() {
        let sysadmin = user(Role::SysAdmin, None);
        assert!(can_administer_user(&sysadmin, Some(42), Some(1)));
        assert!(can_administer_user(&sysadmin, Some(42), None));

        let owner_of_1 = User {
            id: Some(10),
            ..user(Role::TenantOwner, Some(1))
        };
        assert!(can_administer_user(&owner_of_1, Some(99), Some(1))); // same tenant
        assert!(!can_administer_user(&owner_of_1, Some(99), Some(2))); // different tenant
        assert!(can_administer_user(&owner_of_1, Some(10), Some(1))); // self, via the tenant rule

        // DEF-IA-03/HRMS-117: a tenant user administers nobody -- themselves
        // included. Their own account is reachable through change-password,
        // which is a different capability.
        let member_of_1 = User {
            id: Some(20),
            ..user(Role::TenantUser, Some(1))
        };
        assert!(!can_administer_user(&member_of_1, Some(20), Some(1)));
        assert!(!can_administer_user(&member_of_1, Some(99), Some(1)));
    }

    #[test]
    fn reading_your_own_record_is_allowed_where_administering_it_is_not() {
        let member_of_1 = User {
            id: Some(20),
            ..user(Role::TenantUser, Some(1))
        };
        assert!(can_read_user_record(&member_of_1, Some(20), Some(1)));
        assert!(!can_read_user_record(&member_of_1, Some(99), Some(1)));
        assert!(!can_read_user_record(&member_of_1, Some(99), Some(2)));

        // And it stays a superset of the administration rule everywhere else.
        let sysadmin = user(Role::SysAdmin, None);
        assert!(can_read_user_record(&sysadmin, Some(42), Some(1)));
        let owner_of_1 = User {
            id: Some(10),
            ..user(Role::TenantOwner, Some(1))
        };
        assert!(can_read_user_record(&owner_of_1, Some(99), Some(1)));
        assert!(!can_read_user_record(&owner_of_1, Some(99), Some(2)));
    }

    /// EPIC-FO-01-S04 (HRMS-923): the role x operation matrix for the vehicle
    /// register. Every combination is asserted, including the cross-tenant
    /// cells -- those are the ones D-09 exists for, and the ones the endpoint
    /// turns into 404 rather than 403 (PD-034).
    #[test]
    fn vehicle_administration_matrix() {
        let sysadmin = user(Role::SysAdmin, None);
        assert!(can_administer_vehicle(&sysadmin, Some(1)));
        assert!(can_administer_vehicle(&sysadmin, Some(999)));

        let owner_of_1 = user(Role::TenantOwner, Some(1));
        assert!(can_administer_vehicle(&owner_of_1, Some(1)));
        assert!(!can_administer_vehicle(&owner_of_1, Some(2)));
        assert!(!can_administer_vehicle(&owner_of_1, None));

        // HRMS-117's rule read one level down: a tenant user administers no
        // record, a vehicle in their own tenant included.
        let member_of_1 = user(Role::TenantUser, Some(1));
        assert!(!can_administer_vehicle(&member_of_1, Some(1)));
        assert!(!can_administer_vehicle(&member_of_1, Some(2)));

        // A tenant-bound SysAdmin is not unbound (U-019 is still open on
        // whether the state should exist) -- it administers nothing.
        let bound_sysadmin = user(Role::SysAdmin, Some(1));
        assert!(!can_administer_vehicle(&bound_sysadmin, Some(1)));

        // An owner with no tenant has no fleet to administer.
        let unbound_owner = user(Role::TenantOwner, None);
        assert!(!can_administer_vehicle(&unbound_owner, None));
        assert!(!can_administer_vehicle(&unbound_owner, Some(1)));
    }

    #[test]
    fn vehicle_creation_matrix() {
        let sysadmin = user(Role::SysAdmin, None);
        assert!(can_create_vehicle(&sysadmin, Some(1)));
        // HRMS-921: an unbound administrator has no tenant of their own, so
        // an unnamed tenant would produce a vehicle nobody owns.
        assert!(!can_create_vehicle(&sysadmin, None));

        let owner_of_1 = user(Role::TenantOwner, Some(1));
        assert!(can_create_vehicle(&owner_of_1, None));
        assert!(can_create_vehicle(&owner_of_1, Some(1)));
        assert!(!can_create_vehicle(&owner_of_1, Some(2)));

        let member_of_1 = user(Role::TenantUser, Some(1));
        assert!(!can_create_vehicle(&member_of_1, None));
        assert!(!can_create_vehicle(&member_of_1, Some(1)));

        let unbound_owner = user(Role::TenantOwner, None);
        assert!(!can_create_vehicle(&unbound_owner, None));
    }

    #[test]
    fn reading_a_vehicle_is_wider_than_administering_it_but_never_crosses_a_tenant() {
        let member_of_1 = user(Role::TenantUser, Some(1));
        assert!(can_read_vehicle(&member_of_1, Some(1)));
        assert!(!can_administer_vehicle(&member_of_1, Some(1)));
        assert!(!can_read_vehicle(&member_of_1, Some(2)));

        let owner_of_1 = user(Role::TenantOwner, Some(1));
        assert!(can_read_vehicle(&owner_of_1, Some(1)));
        assert!(!can_read_vehicle(&owner_of_1, Some(2)));

        let sysadmin = user(Role::SysAdmin, None);
        assert!(can_read_vehicle(&sysadmin, Some(1)));
        assert!(can_read_vehicle(&sysadmin, Some(999)));

        // A row with no owner is readable by nobody but the platform.
        assert!(!can_read_vehicle(&owner_of_1, None));
    }
}
