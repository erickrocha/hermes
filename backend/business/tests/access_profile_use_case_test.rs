use business::domain::access_profile::AccessProfile;
use business::domain::business_error::BusinessError;
use business::domain::enums::Role;
use business::domain::permission::{GRANTABLE, default_permissions};
use business::domain::user::User;
use business::gateway::access_profile_gateway::AccessProfileStore;
use business::use_cases::access_profile_use_case::{
    AccessProfileUseCase, DUPLICATE_NAME, NAME_REQUIRED, PROFILE_DOES_NOT_FIT, PROFILE_IN_USE,
    PROFILE_NOT_FOUND, ROLE_NOT_ELIGIBLE, UNKNOWN_PERMISSION,
};
use sea_orm::prelude::async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct MemoryStore {
    profiles: Arc<Mutex<Vec<AccessProfile>>>,
    user_profiles: Arc<Mutex<HashMap<i64, Option<i64>>>>,
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
            profile.id = Some(profiles.len() as i64 + 1);
            profile.uuid = Some(format!("profile-{}", profile.id.unwrap()));
            profiles.push(profile.clone());
        }
        Ok(profile)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<AccessProfile>, BusinessError> {
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .find(|profile| profile.id == Some(id))
            .cloned())
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

    async fn find_by_key(
        &self,
        name_key: &str,
        tenant_id: Option<i64>,
    ) -> Result<Option<AccessProfile>, BusinessError> {
        Ok(self
            .profiles
            .lock()
            .unwrap()
            .iter()
            .find(|profile| profile.name_key == name_key && profile.tenant_id == tenant_id)
            .cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<AccessProfile>, u64), BusinessError> {
        let profiles = self.profiles.lock().unwrap();
        let total = profiles.len() as u64;
        let start = (page * page_size) as usize;
        Ok((
            profiles
                .iter()
                .skip(start)
                .take(page_size as usize)
                .cloned()
                .collect(),
            total,
        ))
    }

    async fn user_profile_id(&self, user_id: i64) -> Result<Option<i64>, BusinessError> {
        Ok(self
            .user_profiles
            .lock()
            .unwrap()
            .get(&user_id)
            .copied()
            .flatten())
    }

    async fn set_user_profile(
        &self,
        user_id: i64,
        profile_id: Option<i64>,
    ) -> Result<(), BusinessError> {
        self.user_profiles
            .lock()
            .unwrap()
            .insert(user_id, profile_id);
        Ok(())
    }

    async fn users_holding(&self, profile_id: i64) -> Result<u64, BusinessError> {
        Ok(self
            .user_profiles
            .lock()
            .unwrap()
            .values()
            .filter(|held| **held == Some(profile_id))
            .count() as u64)
    }
}

fn profile(name: &str, role: Role, permissions: Vec<String>) -> AccessProfile {
    AccessProfile {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        name: name.into(),
        name_key: String::new(),
        base_role: role,
        active: true,
        permissions,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn user(id: Option<i64>, role: Role, tenant_id: Option<i64>) -> User {
    User {
        id,
        uuid: None,
        name: Some("Taylor".into()),
        email: "taylor@example.com".into(),
        password: String::new(),
        enabled: true,
        tenant_id,
        role,
        cpf: None,
        principal_function: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn create_normalizes_names_and_sorts_unique_catalog_permissions() {
    let code = GRANTABLE[0].code.to_string();
    let use_case = AccessProfileUseCase::new(MemoryStore::default());
    let created = use_case
        .create(profile(
            "  Dispatch Team  ",
            Role::TenantUser,
            vec![code.clone(), code.clone()],
        ))
        .await
        .unwrap();

    assert_eq!(created.name, "Dispatch Team");
    assert_eq!(created.name_key, "dispatch team");
    assert_eq!(created.permissions, [code]);
    assert!(created.id.is_some());
}

#[tokio::test]
async fn create_rejects_empty_names_ineligible_roles_unknown_permissions_and_duplicates() {
    let store = MemoryStore::default();
    let use_case = AccessProfileUseCase::new(store);
    for (candidate, expected) in [
        (profile("   ", Role::TenantUser, vec![]), NAME_REQUIRED),
        (profile("Owner", Role::SysAdmin, vec![]), ROLE_NOT_ELIGIBLE),
        (
            profile(
                "Dispatch",
                Role::TenantUser,
                vec!["not-a-permission".into()],
            ),
            UNKNOWN_PERMISSION,
        ),
    ] {
        assert_eq!(
            use_case.create(candidate).await.unwrap_err().message,
            expected
        );
    }

    use_case
        .create(profile("Dispatch", Role::TenantUser, vec![]))
        .await
        .unwrap();
    assert_eq!(
        use_case
            .create(profile(" dispatch ", Role::TenantUser, vec![]))
            .await
            .unwrap_err()
            .message,
        DUPLICATE_NAME
    );
}

#[tokio::test]
async fn update_blocks_role_or_activation_changes_while_users_hold_the_profile() {
    let store = MemoryStore::default();
    let use_case = AccessProfileUseCase::new(store.clone());
    let created = use_case
        .create(profile("Dispatch", Role::TenantUser, vec![]))
        .await
        .unwrap();
    let id = created.id.unwrap();
    store
        .set_user_profile(7, Some(id))
        .await
        .expect("the user now holds the profile");

    for edited in [
        AccessProfile {
            active: false,
            ..created.clone()
        },
        AccessProfile {
            base_role: Role::Driver,
            ..created.clone()
        },
    ] {
        assert_eq!(
            use_case.update(edited).await.unwrap_err().message,
            PROFILE_IN_USE
        );
    }

    let updated = use_case
        .update(AccessProfile {
            name: "Dispatch North".into(),
            ..created
        })
        .await
        .unwrap();
    assert_eq!(updated.id, Some(id));
    assert_eq!(updated.name_key, "dispatch north");
}

#[tokio::test]
async fn assignment_checks_role_tenant_and_profile_state_and_can_clear_a_profile() {
    let store = MemoryStore::default();
    let use_case = AccessProfileUseCase::new(store.clone());
    let user_account = user(Some(8), Role::TenantUser, Some(42));
    assert_eq!(
        use_case
            .assign(&user_account, Some("missing".into()))
            .await
            .unwrap_err()
            .message,
        PROFILE_NOT_FOUND
    );

    let profile = use_case
        .create(profile("Dispatch", Role::TenantUser, vec![]))
        .await
        .unwrap();
    let uuid = profile.uuid.clone().unwrap();
    store
        .profiles
        .lock()
        .unwrap()
        .iter_mut()
        .find(|current| current.uuid.as_deref() == Some(&uuid))
        .unwrap()
        .active = false;
    assert_eq!(
        use_case
            .assign(&user_account, Some(uuid.clone()))
            .await
            .unwrap_err()
            .message,
        PROFILE_DOES_NOT_FIT
    );
    assert_eq!(
        use_case
            .assign(&user(Some(8), Role::SysAdmin, Some(42)), None)
            .await
            .unwrap_err()
            .message,
        ROLE_NOT_ELIGIBLE
    );
    assert_eq!(
        use_case
            .assign(&user(None, Role::TenantUser, Some(42)), None)
            .await
            .unwrap_err()
            .message,
        PROFILE_NOT_FOUND
    );

    store
        .profiles
        .lock()
        .unwrap()
        .iter_mut()
        .find(|current| current.uuid.as_deref() == Some(&uuid))
        .unwrap()
        .active = true;
    use_case.assign(&user_account, Some(uuid)).await.unwrap();
    assert_eq!(store.user_profile_id(8).await.unwrap(), profile.id);
    use_case.assign(&user_account, None).await.unwrap();
    assert_eq!(store.user_profile_id(8).await.unwrap(), None);
}

#[tokio::test]
async fn effective_permissions_use_owner_grants_profile_grants_or_role_defaults() {
    let store = MemoryStore::default();
    let use_case = AccessProfileUseCase::new(store.clone());
    let owner = use_case
        .effective(&user(Some(1), Role::TenantOwner, Some(42)))
        .await
        .unwrap();
    assert!(
        GRANTABLE
            .iter()
            .all(|permission| owner.permissions.contains(&permission.code.into()))
    );

    let driver = user(Some(2), Role::Driver, Some(42));
    let defaults = use_case.effective(&driver).await.unwrap();
    assert!(
        default_permissions(&Role::Driver)
            .iter()
            .all(|permission| defaults.permissions.contains(&permission.to_string()))
    );
    assert!(defaults.profile_uuid.is_none());

    let code = GRANTABLE[0].code.to_string();
    let profile = use_case
        .create(profile("Driver profile", Role::Driver, vec![code.clone()]))
        .await
        .unwrap();
    store
        .set_user_profile(driver.id.unwrap(), profile.id)
        .await
        .unwrap();
    let effective = use_case.effective(&driver).await.unwrap();
    assert_eq!(effective.profile_uuid, profile.uuid);
    assert_eq!(effective.profile_name.as_deref(), Some("Driver profile"));
    assert!(effective.permissions.contains(&code));

    store
        .profiles
        .lock()
        .unwrap()
        .iter_mut()
        .find(|current| current.id == profile.id)
        .unwrap()
        .active = false;
    let inactive = use_case.effective(&driver).await.unwrap();
    assert!(inactive.profile_name.is_none());
    assert!(
        default_permissions(&Role::Driver)
            .iter()
            .all(|permission| inactive.permissions.contains(&permission.to_string()))
    );
}
