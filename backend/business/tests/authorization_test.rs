//! `C-035` (`HRMS-017`, `HRMS-865…870`): the profile × capability matrix. Every named capability in
//! `domain::authorization` is a composition of "holds the permission" and "reaches the tenant", so the
//! matrix pins, for each system profile, which capabilities it holds inside its own tenant, that none
//! reaches another tenant, and that the platform actor reaches every tenant it names.

use business::domain::access_profile::AccessProfile;
use business::domain::authorization::*;
use business::domain::permission::{self, DRIVER, Grants, MECHANIC, SYS_ADMIN, TENANT_OWNER, TENANT_USER};
use business::domain::user::{Access, User};

const OWN: i64 = 42;
const OTHER: i64 = 7;
/// `C-036`: the root tenant; the platform actor belongs to it.
const ROOT: i64 = 99;

fn actor(key: &str, tenant_id: Option<i64>) -> User {
    User {
        id: Some(1),
        uuid: Some("actor".into()),
        email: format!("{}@example.com", key.to_lowercase()),
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
        access: Access::of_system_profile(key).expect("a system profile"),
    }
}

fn platform() -> User {
    actor(SYS_ADMIN, Some(ROOT))
}

type Tenanted = fn(&User, Option<i64>) -> bool;
type Plain = fn(&User) -> bool;

/// Capabilities that take the record's (or target's) tenant, with the system profiles below the owner
/// that hold them. The owner holds every one; the platform holds every one in any tenant it names.
const TENANTED: &[(&str, Tenanted, &[&str])] = &[
    ("can_administer_vehicle", can_administer_vehicle, &[]),
    ("can_create_vehicle", can_create_vehicle, &[]),
    ("can_read_vehicle", can_read_vehicle, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_record_odometer", can_record_odometer, &[DRIVER]),
    ("can_administer_customer", can_administer_customer, &[]),
    ("can_create_customer", can_create_customer, &[]),
    ("can_read_customer", can_read_customer, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_administer_holiday", can_administer_holiday, &[]),
    ("can_create_holiday", can_create_holiday, &[]),
    ("can_read_holiday", can_read_holiday, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_administer_transport_demand", can_administer_transport_demand, &[]),
    ("can_create_transport_demand", can_create_transport_demand, &[]),
    ("can_read_transport_demand", can_read_transport_demand, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_administer_extra_trip", can_administer_extra_trip, &[]),
    ("can_create_extra_trip", can_create_extra_trip, &[]),
    ("can_read_extra_trip", can_read_extra_trip, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_administer_checklist_template", can_administer_checklist_template, &[]),
    ("can_create_checklist_template", can_create_checklist_template, &[]),
    ("can_read_checklist_template", can_read_checklist_template, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_administer_checklist_run", can_administer_checklist_run, &[]),
    ("can_create_checklist_run", can_create_checklist_run, &[DRIVER]),
    ("can_read_checklist_run", can_read_checklist_run, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_administer_work_order", can_administer_work_order, &[]),
    ("can_create_work_order", can_create_work_order, &[MECHANIC]),
    ("can_read_work_order", can_read_work_order, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_administer_maintenance_plan", can_administer_maintenance_plan, &[MECHANIC]),
    ("can_create_maintenance_plan", can_create_maintenance_plan, &[MECHANIC]),
    ("can_read_maintenance_plan", can_read_maintenance_plan, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_administer_service_type", can_administer_service_type, &[]),
    ("can_create_service_type", can_create_service_type, &[]),
    ("can_read_service_type", can_read_service_type, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_administer_priced_service", can_administer_priced_service, &[]),
    ("can_create_priced_service", can_create_priced_service, &[]),
    ("can_read_priced_service", can_read_priced_service, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_create_preventive_plan", can_create_preventive_plan, &[MECHANIC]),
    ("can_read_preventive_plan", can_read_preventive_plan, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_work_maintenance", can_work_maintenance, &[MECHANIC]),
    ("can_administer_part", can_administer_part, &[MECHANIC]),
    ("can_create_part", can_create_part, &[MECHANIC]),
    ("can_read_part", can_read_part, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_create_stock_movement", can_create_stock_movement, &[MECHANIC]),
    ("can_create_purchase_order", can_create_purchase_order, &[MECHANIC]),
    ("can_administer_purchase_order", can_administer_purchase_order, &[MECHANIC]),
    ("can_read_purchase_order", can_read_purchase_order, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_administer_vehicle_expense", can_administer_vehicle_expense, &[]),
    ("can_create_vehicle_expense", can_create_vehicle_expense, &[]),
    ("can_read_vehicle_expense", can_read_vehicle_expense, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_create_fuel_entry", can_create_fuel_entry, &[MECHANIC]),
    ("can_report_fuel_receipt", can_report_fuel_receipt, &[DRIVER, MECHANIC]),
    ("can_read_fuel_entry", can_read_fuel_entry, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_configure_internal_tank", can_configure_internal_tank, &[]),
    ("can_configure_fuel_gauge", can_configure_fuel_gauge, &[]),
    ("can_read_internal_tank", can_read_internal_tank, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_manage_garage_catalogue", can_manage_garage_catalogue, &[]),
    ("can_operate_garage", can_operate_garage, &[MECHANIC]),
    ("can_call_vehicle_to_base", can_call_vehicle_to_base, &[]),
    ("can_read_garage", can_read_garage, &[TENANT_USER, DRIVER, MECHANIC]),
    ("can_manage_access_profiles", can_manage_access_profiles, &[]),
];

/// Capabilities with no record to compare tenants against.
const PLAIN: &[(&str, Plain, &[&str])] = &[
    ("can_read_tenant_configuration", can_read_tenant_configuration, &[]),
    ("can_read_billing", can_read_billing, &[]),
    ("can_read_cost_reports", can_read_cost_reports, &[TENANT_USER]),
    ("can_read_non_execution", can_read_non_execution, &[TENANT_USER, MECHANIC]),
    ("can_evaluate_non_execution", can_evaluate_non_execution, &[]),
    ("can_list_users", can_list_users, &[]),
];

/// The platform's alone: a tenant profile never holds them, however wide.
const PLATFORM_ONLY: &[(&str, Plain)] = &[
    ("can_create_tenant", can_create_tenant),
    ("can_set_tenant_plan", can_set_tenant_plan),
    ("can_manage_business_plan_catalogue", can_manage_business_plan_catalogue),
    ("can_manage_reference_data", can_manage_reference_data),
    ("can_link_tracker_device", can_link_tracker_device),
    ("can_publish_presence", can_publish_presence),
];

#[test]
fn every_system_profile_holds_exactly_its_capabilities_inside_its_tenant() {
    let owner = actor(TENANT_OWNER, Some(OWN));
    for (name, capability, holders) in TENANTED {
        assert!(capability(&owner, Some(OWN)), "the owner holds {name}");
        assert!(!capability(&owner, Some(OTHER)), "the owner does not reach another tenant for {name}");
        for key in [TENANT_USER, DRIVER, MECHANIC] {
            let user = actor(key, Some(OWN));
            assert_eq!(capability(&user, Some(OWN)), holders.contains(&key), "{key} × {name}");
            assert!(!capability(&user, Some(OTHER)), "{key} does not reach another tenant for {name}");
        }
    }
    for (name, capability, holders) in PLAIN {
        assert!(capability(&owner), "the owner holds {name}");
        for key in [TENANT_USER, DRIVER, MECHANIC] {
            assert_eq!(capability(&actor(key, Some(OWN))), holders.contains(&key), "{key} × {name}");
        }
    }
}

#[test]
fn the_platform_actor_reaches_every_tenant_it_names_and_no_tenant_profile_reaches_the_platform() {
    let platform = platform();
    for (name, capability, _) in TENANTED {
        assert!(capability(&platform, Some(OWN)) && capability(&platform, Some(OTHER)), "platform × {name}");
    }
    for (name, capability, _) in PLAIN {
        assert!(capability(&platform), "platform × {name}");
    }
    for (name, capability) in PLATFORM_ONLY {
        assert!(capability(&platform), "platform × {name}");
        for key in [TENANT_OWNER, TENANT_USER, DRIVER, MECHANIC] {
            assert!(!capability(&actor(key, Some(OWN))), "{key} never holds {name}");
        }
        // A platform profile without a tenant is not an actor at all (`C-036`: every user has a tenant).
        assert!(!capability(&actor(SYS_ADMIN, None)), "a tenant-less platform profile never holds {name}");
    }
}

#[test]
fn creation_needs_a_tenant_the_platform_names_and_a_tenant_actor_owns() {
    let platform = platform();
    let owner = actor(TENANT_OWNER, Some(OWN));
    let homeless = actor(TENANT_OWNER, None);
    for (name, capability) in [
        ("can_create_vehicle", can_create_vehicle as Tenanted),
        ("can_create_customer", can_create_customer),
        ("can_create_holiday", can_create_holiday),
        ("can_create_transport_demand", can_create_transport_demand),
        ("can_create_extra_trip", can_create_extra_trip),
        ("can_create_checklist_template", can_create_checklist_template),
        ("can_create_checklist_run", can_create_checklist_run),
        ("can_create_work_order", can_create_work_order),
        ("can_create_maintenance_plan", can_create_maintenance_plan),
        ("can_create_service_type", can_create_service_type),
        ("can_create_priced_service", can_create_priced_service),
        ("can_create_preventive_plan", can_create_preventive_plan),
        ("can_create_part", can_create_part),
        ("can_create_stock_movement", can_create_stock_movement),
        ("can_create_purchase_order", can_create_purchase_order),
        ("can_create_vehicle_expense", can_create_vehicle_expense),
        ("can_create_fuel_entry", can_create_fuel_entry),
        ("can_report_fuel_receipt", can_report_fuel_receipt),
        ("can_configure_internal_tank", can_configure_internal_tank),
        ("can_configure_fuel_gauge", can_configure_fuel_gauge),
        ("can_manage_garage_catalogue", can_manage_garage_catalogue),
        ("can_operate_garage", can_operate_garage),
        ("can_call_vehicle_to_base", can_call_vehicle_to_base),
    ] {
        assert!(!capability(&platform, None), "the platform must name a tenant for {name}");
        assert!(capability(&owner, None), "an omitted target defaults to the owner's tenant for {name}");
        assert!(!capability(&homeless, None), "a tenant profile without a tenant creates nothing: {name}");
    }
}

#[test]
fn the_structural_rules_tenant_reach_and_the_generic_compositions() {
    let platform = platform();
    let owner = actor(TENANT_OWNER, Some(OWN));
    assert!(is_platform_actor(&platform) && !is_platform_actor(&owner) && !is_platform_actor(&actor(SYS_ADMIN, None)));
    assert!(lacks_required_tenant(&actor(TENANT_USER, None)) && lacks_required_tenant(&actor(SYS_ADMIN, None)));
    assert!(!lacks_required_tenant(&platform) && !lacks_required_tenant(&owner));
    assert!(can_access_tenant(&owner, OWN) && !can_access_tenant(&owner, OTHER) && can_access_tenant(&platform, OTHER));
    assert!(reaches(&owner, Some(OWN)) && !reaches(&owner, Some(OTHER)) && !reaches(&owner, None) && reaches(&platform, None));
    assert!(may_act_on(&owner, "vehicle.update", Some(OWN)) && !may_act_on(&owner, "tenant.child.create", Some(OWN)));
    assert!(may_act_on_any(&owner, &["nothing.here", "vehicle.update"], Some(OWN)));
    assert!(!may_act_on_any(&actor(DRIVER, Some(OWN)), &["vehicle.update", "vehicle.create"], Some(OWN)));
    assert!(may(&owner, "tenant.update") && !may(&actor(TENANT_OWNER, None), "tenant.update") && may(&platform, "tenant.child.create"));
    assert!(may_create_in(&owner, "vehicle.create", Some(OWN)) && !may_create_in(&owner, "vehicle.create", Some(OTHER)));
    assert!(!may_create_in(&actor(DRIVER, Some(OWN)), "vehicle.create", None));
    assert!(can_update_tenant(&owner, OWN) && !can_update_tenant(&owner, OTHER) && can_update_tenant(&platform, OTHER));
}

#[test]
fn user_administration_follows_the_user_permissions_and_the_tenant() {
    let platform = platform();
    let owner = actor(TENANT_OWNER, Some(OWN));
    let mechanic = actor(MECHANIC, Some(OWN));
    let mut clerk = actor(TENANT_USER, Some(OWN));
    clerk.access.grants.grant("user.create", permission::GrantScope::Full);

    assert!(can_administer_user(&owner, Some(2), Some(OWN)) && !can_administer_user(&owner, Some(2), Some(OTHER)));
    assert!(can_administer_user(&platform, Some(2), Some(OTHER)));
    assert!(!can_administer_user(&mechanic, Some(2), Some(OWN)));
    assert!(can_administer_user(&clerk, Some(2), Some(OWN)), "an extra user permission administers users");
    assert!(can_read_user_record(&mechanic, Some(1), Some(OWN)), "reading one's own record is self-service");
    assert!(!can_read_user_record(&mechanic, Some(2), Some(OWN)));
    assert!(can_list_users(&clerk) && !can_list_users(&mechanic));
    assert!(can_manage_access_profiles(&owner, None) && can_manage_access_profiles(&owner, Some(OWN)));
    assert!(!can_manage_access_profiles(&owner, Some(OTHER)) && can_manage_access_profiles(&platform, Some(OTHER)));
    assert!(!can_manage_access_profiles(&clerk, Some(OWN)));
}

/// `administrator`: the *Administrador do Sistema* system profile.
fn profile(administrator: bool, system: bool, tenant_id: Option<i64>, active: bool) -> AccessProfile {
    AccessProfile {
        id: Some(9),
        uuid: Some("p".into()),
        tenant_id,
        name: "P".into(),
        name_key: "p".into(),
        system,
        key: administrator.then(|| SYS_ADMIN.to_string()),
        active,
        permissions: Vec::new(),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[test]
fn profile_placement_fit_moves_and_the_grant_guard() {
    let platform = platform();
    let owner = actor(TENANT_OWNER, Some(OWN));
    let platform_profile = profile(true, true, None, true);
    let system_tenant = profile(false, true, None, true);
    let own = profile(false, false, Some(OWN), true);
    let foreign = profile(false, false, Some(OTHER), true);
    let inactive = profile(false, false, Some(OWN), false);

    assert_eq!(can_assign_profile(&platform, &platform_profile), Some(CreationTenant::Fixed(Some(ROOT))));
    assert_eq!(can_assign_profile(&platform, &system_tenant), Some(CreationTenant::CallerChosen));
    assert_eq!(can_assign_profile(&platform, &inactive), None);
    assert_eq!(can_assign_profile(&owner, &platform_profile), None);
    assert_eq!(can_assign_profile(&owner, &system_tenant), Some(CreationTenant::Fixed(Some(OWN))));
    assert_eq!(can_assign_profile(&owner, &own), Some(CreationTenant::Fixed(Some(OWN))));
    assert_eq!(can_assign_profile(&owner, &foreign), None);
    assert_eq!(can_assign_profile(&actor(TENANT_OWNER, None), &own), None);

    assert!(profile_fits(&platform_profile, Some(ROOT)) && !profile_fits(&platform_profile, None));
    assert!(profile_fits(&system_tenant, Some(OTHER)) && !profile_fits(&system_tenant, None));
    assert!(profile_fits(&own, Some(OWN)) && !profile_fits(&own, Some(OTHER)) && !profile_fits(&inactive, Some(OWN)));

    assert!(can_move_user_to_tenant(&platform, &own, Some(OWN)) && !can_move_user_to_tenant(&platform, &own, Some(OTHER)));
    assert!(!can_move_user_to_tenant(&owner, &own, Some(OWN)));

    let driver_grants = Grants::from_codes(permission::system_grants(DRIVER));
    assert!(can_grant(&owner, &driver_grants) && can_grant(&platform, &driver_grants));
    assert!(!can_grant(&actor(MECHANIC, Some(OWN)), &Grants::from_codes(["user.create"])));
    assert!(!can_grant(&owner, &Grants::from_codes(["tenant.child.create"])));
}

#[test]
fn a_driver_is_whoever_holds_driver_act_enabled_in_the_tenant() {
    let driver = actor(DRIVER, Some(OWN));
    assert!(is_active_driver_of(&driver, Some(OWN)) && !is_active_driver_of(&driver, Some(OTHER)));
    assert!(!is_active_driver_of(&actor(MECHANIC, Some(OWN)), Some(OWN)));
    assert!(!is_active_driver_of(&actor(TENANT_USER, Some(OWN)), Some(OWN)));
    let mut off = actor(DRIVER, Some(OWN));
    off.enabled = false;
    assert!(!is_active_driver_of(&off, Some(OWN)));
    let mut clerk = actor(TENANT_USER, Some(OWN));
    clerk.access.grants.grant(permission::DRIVER_ACT, permission::GrantScope::Full);
    assert!(is_active_driver_of(&clerk, Some(OWN)), "an extra driver.act makes a driver of anyone");
    assert!(!is_active_driver_of(&actor(DRIVER, None), None), "a driver without a tenant drives nowhere");
}

/// `C-036`: the platform actor now belongs to the root tenant, but it is still not confined to one -- a request
/// it makes acts in the tenant it names, and everyone else acts in their own.
#[test]
fn the_platform_actor_is_not_confined_to_its_own_tenant_and_names_the_one_it_acts_in() {
    let platform = platform();
    let owner = actor(TENANT_OWNER, Some(OWN));
    assert_eq!(platform.bound_tenant(), None);
    assert_eq!(owner.bound_tenant(), Some(OWN));
    assert_eq!(platform.target_tenant(Some(OTHER)), Some(OTHER));
    assert_eq!(platform.target_tenant(None), None, "the platform must name a tenant");
    assert_eq!(owner.target_tenant(Some(OTHER)), Some(OWN), "a tenant account cannot name another tenant");
}
