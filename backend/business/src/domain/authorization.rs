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
/// tenant owner creates tenant users in their own tenant only. Every
/// other combination -- including a platform administrator creating a
/// tenant user directly, which is the tenant owner's job -- is rejected
/// (`None`), per S05's "every combination outside the hierarchy is
/// rejected."
pub fn can_create_user_with_role(actor: &User, target_role: &Role) -> Option<CreationTenant> {
    if is_unbound_sys_admin(actor) {
        match target_role {
            Role::SysAdmin => Some(CreationTenant::Fixed(None)),
            Role::TenantOwner => Some(CreationTenant::CallerChosen),
            Role::TenantUser => None,
        }
    } else if actor.role == Role::TenantOwner {
        match target_role {
            Role::TenantUser => actor.tenant_id.map(|id| CreationTenant::Fixed(Some(id))),
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
pub fn can_reassign_role(actor: &User, requested_role: &Role, requested_tenant_id: Option<i64>) -> bool {
    is_unbound_sys_admin(actor)
        && match requested_role {
            Role::SysAdmin => requested_tenant_id.is_none(),
            Role::TenantOwner => requested_tenant_id.is_some(),
            Role::TenantUser => false,
        }
}

/// Whether `actor` may administer `target`'s record at all (EPIC-IA-05-S01,
/// HRMS-118): the target editing themselves, an unbound platform
/// administrator (any target), or a tenant owner whose tenant matches the
/// target's exactly.
pub fn can_administer_user(actor: &User, target_id: Option<i64>, target_tenant_id: Option<i64>) -> bool {
    (actor.id.is_some() && actor.id == target_id)
        || is_unbound_sys_admin(actor)
        || (actor.role == Role::TenantOwner && actor.tenant_id == target_tenant_id)
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
        assert_eq!(can_create_user_with_role(&sysadmin, &Role::TenantUser), None);

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

    #[test]
    fn can_administer_user_matrix() {
        let sysadmin = user(Role::SysAdmin, None);
        assert!(can_administer_user(&sysadmin, Some(42), Some(1)));
        assert!(can_administer_user(&sysadmin, Some(42), None));

        let owner_of_1 = User { id: Some(10), ..user(Role::TenantOwner, Some(1)) };
        assert!(can_administer_user(&owner_of_1, Some(99), Some(1))); // same tenant
        assert!(!can_administer_user(&owner_of_1, Some(99), Some(2))); // different tenant
        assert!(can_administer_user(&owner_of_1, Some(10), Some(1))); // self

        let member_of_1 = User { id: Some(20), ..user(Role::TenantUser, Some(1)) };
        assert!(can_administer_user(&member_of_1, Some(20), Some(1))); // self only
        assert!(!can_administer_user(&member_of_1, Some(99), Some(1))); // not self, no admin rights
    }
}
