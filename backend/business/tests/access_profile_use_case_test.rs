use business::domain::access_profile::AccessProfile;
use business::domain::business_error::BusinessError;
use business::domain::enums::ProfileScope;
use business::domain::permission::{
    DRIVER, DRIVER_ACT, MECHANIC, SYS_ADMIN, SYSTEM_PROFILES, TENANT_OWNER, TENANT_USER, system_grants,
};
use business::domain::user::{Access, User};
use business::gateway::access_profile_gateway::AccessProfileStore;
use business::use_cases::access_profile_use_case::{
    AccessProfileUseCase, DUPLICATE_NAME, GRANT_EXCEEDS_ACTOR, NAME_REQUIRED, PROFILE_DOES_NOT_FIT, PROFILE_IN_USE,
    PROFILE_NOT_FOUND, SCOPE_NOT_ALLOWED, SYSTEM_PROFILE_READ_ONLY, UNKNOWN_PERMISSION,
};
use sea_orm::prelude::async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// A user's held profile and extra grants, as `set_user_access` writes them.
type HeldAccess = (i64, Vec<String>);

#[derive(Clone, Default)]
struct MemoryStore {
    profiles: Arc<Mutex<Vec<AccessProfile>>>,
    user_access: Arc<Mutex<HashMap<i64, HeldAccess>>>,
}

impl MemoryStore {
    fn set_active(&self, uuid: &str, active: bool) {
        let mut profiles = self.profiles.lock().unwrap();
        let profile = profiles
            .iter_mut()
            .find(|current| current.uuid.as_deref() == Some(uuid))
            .expect("profile exists");
        profile.active = active;
    }
}

#[async_trait]
impl AccessProfileStore for MemoryStore {
    async fn save(&self, mut profile: AccessProfile) -> Result<AccessProfile, BusinessError> {
        let mut profiles = self.profiles.lock().unwrap();
        if let Some(id) = profile.id {
            if let Some(existing) = profiles.iter_mut().find(|current| current.id == Some(id)) {
                profile.uuid = existing.uuid.clone();
                *existing = profile.clone();
            } else {
                profiles.push(profile.clone());
            }
        } else {
            let id = profiles.len() as i64 + 1;
            profile.id = Some(id);
            profile.uuid = Some(format!("profile-{id}"));
            profiles.push(profile.clone());
        }
        Ok(profile)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<AccessProfile>, BusinessError> {
        Ok(self.profiles.lock().unwrap().iter().find(|profile| profile.id == Some(id)).cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<AccessProfile>, BusinessError> {
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .find(|profile| profile.uuid.as_deref() == Some(&uuid))
            .cloned())
    }

    async fn find_by_name_key(&self, name_key: &str, tenant_id: Option<i64>) -> Result<Option<AccessProfile>, BusinessError> {
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .find(|profile| profile.name_key == name_key && profile.tenant_id == tenant_id)
            .cloned())
    }

    async fn find_by_key(&self, key: &str) -> Result<Option<AccessProfile>, BusinessError> {
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .find(|profile| profile.key.as_deref() == Some(key))
            .cloned())
    }

    async fn list_page(&self, page: u64, page_size: u64) -> Result<(Vec<AccessProfile>, u64), BusinessError> {
        let profiles = self.profiles.lock().unwrap();
        let total = profiles.len() as u64;
        let start = (page * page_size) as usize;
        Ok((profiles.iter().skip(start).take(page_size as usize).cloned().collect(), total))
    }

    async fn set_user_access(&self, user_id: i64, profile_id: i64, extras: &[String]) -> Result<(), BusinessError> {
        self.user_access.lock().unwrap().insert(user_id, (profile_id, extras.to_vec()));
        Ok(())
    }

    async fn users_holding(&self, profile_id: i64) -> Result<u64, BusinessError> {
        Ok(self
            .user_access
            .lock()
            .unwrap()
            .values()
            .filter(|(held, _)| *held == profile_id)
            .count() as u64)
    }
}

fn profile(name: &str, tenant_id: Option<i64>, scope: ProfileScope, permissions: Vec<&str>) -> AccessProfile {
    AccessProfile {
        id: None,
        uuid: None,
        tenant_id,
        name: name.into(),
        name_key: String::new(),
        scope,
        system: false,
        key: None,
        active: true,
        permissions: permissions.into_iter().map(str::to_string).collect(),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn tenant_profile(name: &str, permissions: Vec<&str>) -> AccessProfile {
    profile(name, Some(42), ProfileScope::Tenant, permissions)
}

fn user(id: Option<i64>, profile_key: &str, tenant_id: Option<i64>) -> User {
    User {
        id,
        uuid: None,
        name: Some("Taylor".into()),
        email: "taylor@example.com".into(),
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
        access: Access::of_system_profile(profile_key).expect("a system profile"),
    }
}

fn owner() -> User {
    user(Some(1), TENANT_OWNER, Some(42))
}

fn platform() -> User {
    user(Some(2), SYS_ADMIN, None)
}

async fn seeded() -> (MemoryStore, AccessProfileUseCase<MemoryStore>) {
    let store = MemoryStore::default();
    let use_case = AccessProfileUseCase::new(store.clone());
    use_case.sync_system_profiles().await.expect("system profiles seeded");
    (store, use_case)
}

#[tokio::test]
async fn create_normalizes_names_and_grants_for_a_tenant_profile() {
    let use_case = AccessProfileUseCase::new(MemoryStore::default());
    let created = use_case
        .create(
            &owner(),
            tenant_profile("  Dispatch Team  ", vec!["vehicle.read", "vehicle.read@own", "vehicle.read", "fuel-entry.read@own"]),
        )
        .await
        .unwrap();

    assert_eq!(created.name, "Dispatch Team");
    assert_eq!(created.name_key, "dispatch team");
    assert_eq!(created.permissions, ["fuel-entry.read@own", "vehicle.read"]);
    assert_eq!(created.scope, ProfileScope::Tenant);
    assert_eq!(created.tenant_id, Some(42));
    assert!(!created.system && created.key.is_none());
    assert!(created.id.is_some());
}

#[tokio::test]
async fn create_rejects_empty_names_bad_grants_scope_breaches_and_duplicates() {
    let use_case = AccessProfileUseCase::new(MemoryStore::default());
    let owner = owner();
    for (candidate, expected) in [
        (tenant_profile("   ", vec![]), NAME_REQUIRED),
        (tenant_profile("Dispatch", vec!["not-a-permission"]), UNKNOWN_PERMISSION),
        // `@own` on a permission that has no own scope
        (tenant_profile("Dispatch", vec!["vehicle.create@own"]), UNKNOWN_PERMISSION),
        // a platform permission has no place in a tenant profile
        (tenant_profile("Dispatch", vec!["tenant.create"]), UNKNOWN_PERMISSION),
        // a tenant actor neither creates platform profiles nor another tenant's
        (profile("Platform ops", None, ProfileScope::Platform, vec![]), SCOPE_NOT_ALLOWED),
        (profile("Elsewhere", Some(99), ProfileScope::Tenant, vec![]), SCOPE_NOT_ALLOWED),
        (profile("Nowhere", None, ProfileScope::Tenant, vec![]), SCOPE_NOT_ALLOWED),
    ] {
        assert_eq!(use_case.create(&owner, candidate).await.unwrap_err().message, expected);
    }

    use_case.create(&owner, tenant_profile("Dispatch", vec![])).await.unwrap();
    assert_eq!(
        use_case.create(&owner, tenant_profile(" dispatch ", vec![])).await.unwrap_err().message,
        DUPLICATE_NAME
    );
}

#[tokio::test]
async fn nobody_grants_what_they_do_not_hold() {
    let use_case = AccessProfileUseCase::new(MemoryStore::default());
    let tenant_user = user(Some(3), TENANT_USER, Some(42));
    assert_eq!(
        use_case
            .create(&tenant_user, tenant_profile("Admins", vec!["user.create"]))
            .await
            .unwrap_err()
            .message,
        GRANT_EXCEEDS_ACTOR
    );
    // an own-scoped grant does not cover the full one
    let mut driver = user(Some(4), DRIVER, Some(42));
    driver.extra_permissions = vec!["profile.create".into()];
    driver.access = Access::from_grants(ProfileScope::Tenant, ["vehicle.read@own", "profile.create"]);
    assert_eq!(
        use_case.create(&driver, tenant_profile("Readers", vec!["vehicle.read"])).await.unwrap_err().message,
        GRANT_EXCEEDS_ACTOR
    );
    let created = use_case.create(&driver, tenant_profile("Readers", vec!["vehicle.read@own"])).await.unwrap();
    assert_eq!(created.permissions, ["vehicle.read@own"]);
}

#[tokio::test]
async fn the_platform_creates_platform_profiles_and_tenant_profiles_naming_the_tenant() {
    let use_case = AccessProfileUseCase::new(MemoryStore::default());
    let platform = platform();
    let ops = use_case
        .create(&platform, profile("Platform ops", None, ProfileScope::Platform, vec!["tenant.create", "vehicle.read"]))
        .await
        .unwrap();
    assert_eq!(ops.scope, ProfileScope::Platform);
    assert_eq!(ops.tenant_id, None);
    assert_eq!(ops.permissions, ["tenant.create", "vehicle.read"]);

    let named = use_case.create(&platform, profile("Dispatch", Some(7), ProfileScope::Tenant, vec![])).await.unwrap();
    assert_eq!(named.tenant_id, Some(7));
    assert_eq!(
        use_case
            .create(&platform, profile("Stamped", Some(7), ProfileScope::Platform, vec![]))
            .await
            .unwrap_err()
            .message,
        SCOPE_NOT_ALLOWED
    );
}

#[tokio::test]
async fn update_keeps_system_profiles_read_only_and_held_profiles_active() {
    let (store, use_case) = seeded().await;
    let owner = owner();
    let driver = use_case.find_by_key(DRIVER).await.unwrap();
    assert_eq!(
        use_case
            .update(&owner, AccessProfile { name: "Pilot".into(), ..driver })
            .await
            .unwrap_err()
            .message,
        SYSTEM_PROFILE_READ_ONLY
    );

    let created = use_case.create(&owner, tenant_profile("Dispatch", vec!["vehicle.read"])).await.unwrap();
    let id = created.id.unwrap();
    store.set_user_access(7, id, &[]).await.expect("the user now holds the profile");
    assert_eq!(
        use_case
            .update(&owner, AccessProfile { active: false, ..created.clone() })
            .await
            .unwrap_err()
            .message,
        PROFILE_IN_USE
    );
    assert_eq!(
        use_case
            .update(&owner, AccessProfile { uuid: Some("missing".into()), ..created.clone() })
            .await
            .unwrap_err()
            .message,
        PROFILE_NOT_FOUND
    );

    // the identity, tenant and scope stay what they were; name and grants follow the edit
    let updated = use_case
        .update(
            &owner,
            AccessProfile {
                name: "Dispatch North".into(),
                tenant_id: Some(99),
                scope: ProfileScope::Platform,
                system: true,
                key: Some("Hacked".into()),
                permissions: vec!["vehicle.read".into(), "vehicle.update".into()],
                ..created
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.id, Some(id));
    assert_eq!(updated.name_key, "dispatch north");
    assert_eq!(updated.tenant_id, Some(42));
    assert_eq!(updated.scope, ProfileScope::Tenant);
    assert!(!updated.system && updated.key.is_none());
    assert_eq!(updated.permissions, ["vehicle.read", "vehicle.update"]);
}

#[tokio::test]
async fn assignment_requires_a_fitting_profile_within_reach_and_normalises_the_extras() {
    let (store, use_case) = seeded().await;
    let owner = owner();
    let account = user(Some(8), TENANT_USER, Some(42));
    assert_eq!(
        use_case.assign(&owner, &account, "missing".into(), &[]).await.unwrap_err().message,
        PROFILE_NOT_FOUND
    );
    assert_eq!(
        use_case
            .assign(&owner, &user(None, TENANT_USER, Some(42)), "profile-1".into(), &[])
            .await
            .unwrap_err()
            .message,
        PROFILE_NOT_FOUND
    );

    let dispatch = use_case.create(&owner, tenant_profile("Dispatch", vec!["vehicle.read"])).await.unwrap();
    let uuid = dispatch.uuid.clone().unwrap();
    store.set_active(&uuid, false);
    assert_eq!(
        use_case.assign(&owner, &account, uuid.clone(), &[]).await.unwrap_err().message,
        PROFILE_DOES_NOT_FIT
    );
    store.set_active(&uuid, true);

    // another tenant's user, and a platform profile for a tenant-bound account
    assert_eq!(
        use_case
            .assign(&owner, &user(Some(9), TENANT_USER, Some(99)), uuid.clone(), &[])
            .await
            .unwrap_err()
            .message,
        PROFILE_DOES_NOT_FIT
    );
    let sysadmin = use_case.find_by_key(SYS_ADMIN).await.unwrap();
    assert_eq!(
        use_case
            .assign(&owner, &account, sysadmin.uuid.clone().unwrap(), &[])
            .await
            .unwrap_err()
            .message,
        PROFILE_DOES_NOT_FIT
    );
    // the platform gives out platform profiles only to accounts with no tenant
    assert_eq!(
        use_case
            .assign(&platform(), &account, sysadmin.uuid.clone().unwrap(), &[])
            .await
            .unwrap_err()
            .message,
        PROFILE_DOES_NOT_FIT
    );
    use_case
        .assign(&platform(), &user(Some(10), TENANT_USER, None), sysadmin.uuid.clone().unwrap(), &[])
        .await
        .unwrap();
    assert_eq!(store.user_access.lock().unwrap().get(&10), Some(&(sysadmin.id.unwrap(), vec![])));

    // extras: validated against the catalogue, normalised, and covered by the actor
    assert_eq!(
        use_case
            .assign(&owner, &account, uuid.clone(), &["wizard.cast".into()])
            .await
            .unwrap_err()
            .message,
        UNKNOWN_PERMISSION
    );
    assert_eq!(
        use_case
            .assign(&owner, &account, uuid.clone(), &["tenant.create".into()])
            .await
            .unwrap_err()
            .message,
        UNKNOWN_PERMISSION
    );
    assert_eq!(
        use_case
            .assign(&user(Some(11), TENANT_USER, Some(42)), &account, uuid.clone(), &["user.create".into()])
            .await
            .unwrap_err()
            .message,
        GRANT_EXCEEDS_ACTOR
    );
    use_case
        .assign(
            &owner,
            &account,
            uuid.clone(),
            &["fuel-entry.read@own".into(), "fuel-entry.read".into(), "vehicle.update".into()],
        )
        .await
        .unwrap();
    assert_eq!(
        store.user_access.lock().unwrap().get(&8),
        Some(&(dispatch.id.unwrap(), vec!["fuel-entry.read".to_string(), "vehicle.update".to_string()]))
    );

    // a system tenant profile fits any tenant; the owner may hand it out inside their own
    let driver = use_case.find_by_key(DRIVER).await.unwrap();
    let (placed, extras) = use_case.placeable(&owner, driver.uuid.clone().unwrap(), Some(42), &[]).await.unwrap();
    assert_eq!(placed.key.as_deref(), Some(DRIVER));
    assert!(extras.is_empty());
}

#[tokio::test]
async fn effective_permissions_combine_profile_extras_and_baseline() {
    let use_case = AccessProfileUseCase::new(MemoryStore::default());
    let dispatch = AccessProfile { id: Some(5), uuid: Some("profile-5".into()), ..tenant_profile("Dispatch", vec!["vehicle.read@own"]) };
    let mut account = user(Some(8), TENANT_USER, Some(42));
    account.extra_permissions = vec!["fuel-entry.read".into()];
    account.access = Access::resolve(&dispatch, &account.extra_permissions);

    let effective = use_case.effective(&account);
    assert_eq!(effective.scope, ProfileScope::Tenant);
    assert_eq!(effective.profile_uuid.as_deref(), Some("profile-5"));
    assert_eq!(effective.profile_name.as_deref(), Some("Dispatch"));
    assert!(effective.profile_key.is_none());
    assert_eq!(effective.extra_permissions, ["fuel-entry.read"]);
    for grant in ["vehicle.read@own", "fuel-entry.read", "user.read-own"] {
        assert!(effective.permissions.contains(&grant.to_string()), "{grant}");
    }
    assert!(!effective.permissions.contains(&"vehicle.read".to_string()));

    // an inactive profile grants nothing beyond the baseline and the extras
    account.access = Access::resolve(&AccessProfile { active: false, ..dispatch }, &account.extra_permissions);
    let inactive = use_case.effective(&account);
    assert_eq!(inactive.profile_name.as_deref(), Some("Dispatch"));
    assert!(!inactive.permissions.iter().any(|g| g.starts_with("vehicle.read")));
    assert!(inactive.permissions.contains(&"fuel-entry.read".to_string()));
    assert!(inactive.permissions.contains(&"user.read-own".to_string()));

    let driver = use_case.effective(&user(Some(2), DRIVER, Some(42)));
    assert_eq!(driver.profile_key.as_deref(), Some(DRIVER));
    assert!(driver.permissions.contains(&DRIVER_ACT.to_string()));
    assert!(system_grants(DRIVER).iter().all(|grant| driver.permissions.contains(grant)));
}

#[tokio::test]
async fn sync_creates_missing_system_profiles_and_resyncs_drifted_ones() {
    let store = MemoryStore::default();
    let use_case = AccessProfileUseCase::new(store.clone());
    let sysadmin_id = use_case.sync_system_profiles().await.unwrap();

    let profiles = store.profiles.lock().unwrap().clone();
    assert_eq!(profiles.len(), SYSTEM_PROFILES.len());
    for system in SYSTEM_PROFILES {
        let saved = profiles.iter().find(|p| p.key.as_deref() == Some(system.key)).expect(system.key);
        assert!(saved.system && saved.active && saved.tenant_id.is_none());
        assert_eq!(saved.scope, system.scope);
        assert_eq!(saved.name, system.name);
        assert_eq!(saved.permissions, system_grants(system.key));
    }
    let sysadmin = profiles.iter().find(|p| p.key.as_deref() == Some(SYS_ADMIN)).unwrap();
    assert_eq!(Some(sysadmin_id), sysadmin.id);
    assert_eq!(sysadmin.scope, ProfileScope::Platform);

    // drift: grants edited by hand and the profile deactivated are put back
    {
        let mut profiles = store.profiles.lock().unwrap();
        let driver = profiles.iter_mut().find(|p| p.key.as_deref() == Some(DRIVER)).unwrap();
        driver.permissions = vec!["vehicle.read".into()];
        driver.active = false;
    }
    assert_eq!(use_case.sync_system_profiles().await.unwrap(), sysadmin_id);
    let after = store.profiles.lock().unwrap().clone();
    assert_eq!(after.len(), SYSTEM_PROFILES.len(), "a resync adds no rows");
    let driver = after.iter().find(|p| p.key.as_deref() == Some(DRIVER)).unwrap();
    assert!(driver.active);
    assert_eq!(driver.permissions, system_grants(DRIVER));
    assert_eq!(driver.uuid.as_deref(), Some("profile-4"), "the row keeps its identity");

    let found = use_case.find_by_key(TENANT_OWNER).await.unwrap();
    assert_eq!(found.name, "Tenant owner");
    assert_eq!(use_case.find_by_key("Nobody").await.unwrap_err().message, PROFILE_NOT_FOUND);
}

#[tokio::test]
async fn operational_system_profiles_are_handed_out_without_the_grant_guard_but_nothing_else_is() {
    let (_, use_case) = seeded().await;
    // an administrator who holds none of the driver's grants beyond user administration
    let mut dispatcher = user(Some(20), TENANT_USER, Some(42));
    dispatcher.access = Access::from_grants(ProfileScope::Tenant, ["user.create", "profile.assign", "vehicle.read"]);
    assert!(!dispatcher.allows(DRIVER_ACT));
    let account = user(Some(21), TENANT_USER, Some(42));

    for key in [DRIVER, TENANT_USER, MECHANIC] {
        let system = use_case.find_by_key(key).await.unwrap();
        let (placed, _) = use_case
            .placeable(&dispatcher, system.uuid.clone().unwrap(), Some(42), &[])
            .await
            .unwrap_or_else(|e| panic!("{key}: {}", e.message));
        assert_eq!(placed.key.as_deref(), Some(key));
    }
    // ... yet the extras on top of one are still theirs to cover
    let driver = use_case.find_by_key(DRIVER).await.unwrap();
    assert_eq!(
        use_case
            .placeable(&dispatcher, driver.uuid.clone().unwrap(), Some(42), &["vehicle.update".into()])
            .await
            .unwrap_err()
            .message,
        GRANT_EXCEEDS_ACTOR
    );
    use_case
        .assign(&dispatcher, &account, driver.uuid.clone().unwrap(), &["vehicle.read@own".into()])
        .await
        .unwrap();

    // the owner's system profile and a tenant-made profile stay guarded
    let owner_profile = use_case.find_by_key(TENANT_OWNER).await.unwrap();
    assert_eq!(
        use_case
            .placeable(&dispatcher, owner_profile.uuid.clone().unwrap(), Some(42), &[])
            .await
            .unwrap_err()
            .message,
        GRANT_EXCEEDS_ACTOR
    );
    let custom = use_case.create(&owner(), tenant_profile("Fleet admins", vec!["vehicle.update"])).await.unwrap();
    assert_eq!(
        use_case
            .placeable(&dispatcher, custom.uuid.clone().unwrap(), Some(42), &[])
            .await
            .unwrap_err()
            .message,
        GRANT_EXCEEDS_ACTOR
    );
    // the platform's profile is out of a tenant actor's reach before the guard is even asked
    let sysadmin = use_case.find_by_key(SYS_ADMIN).await.unwrap();
    let mut platform_user = user(Some(22), TENANT_USER, None);
    platform_user.access = Access::from_grants(ProfileScope::Platform, ["user.create", "profile.assign"]);
    assert_eq!(
        use_case
            .placeable(&platform_user, sysadmin.uuid.clone().unwrap(), None, &[])
            .await
            .unwrap_err()
            .message,
        GRANT_EXCEEDS_ACTOR
    );
}
