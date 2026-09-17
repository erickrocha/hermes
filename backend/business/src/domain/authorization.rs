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
//! Deliberately NOT covered here: `user_endpoint`'s creation-hierarchy and
//! role-reassignment logic (`add`/`update`) is its own, more involved shape
//! -- consolidating it is EPIC-IA-04's job, and folding it in here risks
//! changing behaviour this epic isn't meant to touch. `S05` (whether a
//! tenant-bound administrator is even representable) is left alone too: it
//! is an open decision (U-019, decision-register.md D-08), not a refactor.

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

/// `POST /tenant/{id}/plan` -- only an unbound platform administrator may
/// set a tenant's plan (HRMS-224, PD-019, PD-021).
pub fn can_set_tenant_plan(user: &User) -> bool {
    is_unbound_sys_admin(user)
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
            first_login: false,
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
        }
    }
}
