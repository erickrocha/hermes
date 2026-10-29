use business::commons::password;
use business::domain::business_error::BusinessError;
use business::domain::permission::{SYS_ADMIN, TENANT_USER};
use business::domain::user::{Access, User};
use business::gateway::user_gateway::{UserLookupStore, UserStore};
use business::use_cases::user_use_case::UserUseCase;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct MemoryUserStore(Arc<Mutex<Vec<User>>>);

/// The two profile rows this store knows, standing in for the `access_profile` table.
const TENANT_USER_PROFILE_ID: i64 = 1;
const SYSADMIN_PROFILE_ID: i64 = 9;

/// What `UserGateway` does on every read: resolve the account's access from its profile.
fn resolved(mut user: User) -> User {
    let key = match user.profile_id {
        Some(SYSADMIN_PROFILE_ID) => SYS_ADMIN,
        _ => TENANT_USER,
    };
    user.access = Access::of_system_profile(key).expect("a system profile");
    user
}

#[async_trait]
impl UserLookupStore for MemoryUserStore {
    async fn get_by_id(&self, id: i64) -> Result<Option<User>, BusinessError> {
        let users = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(users.iter().find(|user| user.id == Some(id)).cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<User>, BusinessError> {
        let users = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(users
            .iter()
            .find(|user| user.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }
}

#[async_trait]
impl UserStore for MemoryUserStore {
    async fn save(&self, user: User) -> Result<User, BusinessError> {
        let mut users = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let mut user = resolved(user);
        match user.id {
            Some(id) => {
                if let Some(existing) = users.iter_mut().find(|existing| existing.id == Some(id)) {
                    *existing = user.clone();
                } else {
                    users.push(user.clone());
                }
            }
            None => {
                let id = users.len() as i64 + 1;
                user.id = Some(id);
                user.uuid = Some(format!("user-{id}"));
                users.push(user.clone());
            }
        }
        Ok(user)
    }

    async fn get_by_email(&self, email: String) -> Result<Option<User>, BusinessError> {
        let users = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(users.iter().find(|user| user.email == email).cloned())
    }

    async fn get_by_profile_key(&self, key: &str) -> Result<Option<User>, BusinessError> {
        let users = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(users
            .iter()
            .find(|user| user.access.profile_key.as_deref() == Some(key))
            .cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<User>, u64), BusinessError> {
        let users = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let filtered: Vec<_> = users
            .iter()
            .filter(|user| {
                search.is_none_or(|term| {
                    user.email.contains(term)
                        || user.name.as_deref().is_some_and(|name| name.contains(term))
                })
            })
            .cloned()
            .collect();
        let total = filtered.len() as u64;
        let start = (page * page_size) as usize;
        Ok((
            filtered
                .into_iter()
                .skip(start)
                .take(page_size as usize)
                .collect(),
            total,
        ))
    }

    async fn list_all(&self) -> Result<Vec<User>, BusinessError> {
        let users = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(users.clone())
    }

    async fn list_by_tenant(&self, tenant_id: i64) -> Result<Vec<User>, BusinessError> {
        let users = self
            .0
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(users
            .iter()
            .filter(|user| user.tenant_id == Some(tenant_id))
            .cloned()
            .collect())
    }
}

fn new_user(email: &str, password: &str) -> User {
    User {
        id: None,
        uuid: None,
        email: email.into(),
        name: Some("Example user".into()),
        password: password.into(),
        enabled: true,
        tenant_id: Some(42),
        profile_id: Some(TENANT_USER_PROFILE_ID),
        extra_permissions: Vec::new(),
        cpf: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
        access: Access::of_system_profile(TENANT_USER).expect("a system profile"),
    }
}

fn use_case() -> UserUseCase {
    UserUseCase::new(MemoryUserStore::default())
}

fn use_case_with_store(store: MemoryUserStore) -> UserUseCase {
    UserUseCase::new(store)
}

#[tokio::test]
async fn user_use_case_hashes_passwords_and_reads_from_its_store_port() {
    let use_case = use_case();
    let created = use_case
        .create(new_user("user@example.com", "Correct#Horse9"))
        .await
        .expect("valid user is created");
    let found = use_case
        .find_by_email("user@example.com".into())
        .await
        .expect("email lookup succeeds")
        .expect("user exists");

    assert_ne!(created.password, "Correct#Horse9");
    assert!(password::verify("Correct#Horse9", &created.password));
    assert_eq!(found.id, created.id);
}

#[tokio::test]
async fn user_update_preserves_the_existing_password() {
    let use_case = use_case();
    let created = use_case
        .create(new_user("user@example.com", "Correct#Horse9"))
        .await
        .expect("user is created");
    let mut requested = new_user("updated@example.com", "Ignored#Password2");
    requested.id = created.id;

    let updated = use_case
        .update(created.id.expect("store assigned id"), requested)
        .await
        .expect("user is updated");

    assert_eq!(updated.email, "updated@example.com");
    assert_eq!(updated.password, created.password);
}

#[tokio::test]
async fn user_creation_and_persist_validate_required_fields() {
    let use_case = use_case();
    assert_eq!(
        use_case
            .create(new_user(" ", "Correct#Horse9"))
            .await
            .expect_err("email is required")
            .message,
        "User email is required"
    );
    assert_eq!(
        use_case
            .create(new_user("user@example.com", " "))
            .await
            .expect_err("password is required")
            .message,
        "User password is required"
    );
    assert!(
        use_case
            .create(new_user("user@example.com", "short"))
            .await
            .is_err(),
        "password policy applies to newly created users"
    );
    let mut without_profile = new_user("user@example.com", "Correct#Horse9");
    without_profile.profile_id = None;
    assert_eq!(
        use_case
            .create(without_profile)
            .await
            .expect_err("an account always holds a profile")
            .message,
        "User profile is required"
    );
    let mut unknown_grant = new_user("user@example.com", "Correct#Horse9");
    unknown_grant.extra_permissions = vec!["wizard.cast".into()];
    assert_eq!(
        use_case
            .create(unknown_grant)
            .await
            .expect_err("extras come from the catalogue")
            .message,
        "Unknown permission: wizard.cast"
    );
    // `C-036`: one catalogue -- whether the actor may hand out a given permission is `can_grant`'s question, asked by
    // the endpoint; here only the shape is checked.
    let mut platform_grant = new_user("user@example.com", "Correct#Horse9");
    platform_grant.extra_permissions = vec!["business-plan.read".into()];
    assert!(use_case.create(platform_grant).await.is_ok());
    assert_eq!(
        use_case
            .persist(new_user("", "secret"))
            .await
            .expect_err("persist requires email")
            .message,
        "Email and password are required"
    );
}

#[tokio::test]
async fn user_extra_permissions_are_normalised_on_create_and_kept_on_update() {
    let use_case = use_case();
    let mut requested = new_user("user@example.com", "Correct#Horse9");
    requested.extra_permissions = vec!["vehicle.read@own".into(), "vehicle.read".into(), "fuel-entry.read@own".into()];
    let created = use_case.create(requested).await.expect("user is created");
    assert_eq!(created.extra_permissions, ["fuel-entry.read@own", "vehicle.read"]);

    // an update without a profile keeps the one held; its extras are validated the same way
    let mut edited = new_user("user@example.com", "");
    edited.profile_id = None;
    edited.extra_permissions = vec!["vehicle.update".into()];
    let updated = use_case
        .update(created.id.expect("store assigned id"), edited)
        .await
        .expect("user is updated");
    assert_eq!(updated.profile_id, Some(TENANT_USER_PROFILE_ID));
    assert_eq!(updated.extra_permissions, ["vehicle.update"]);

    let mut bad = new_user("user@example.com", "");
    bad.extra_permissions = vec!["nope".into()];
    assert_eq!(
        use_case
            .update(created.id.expect("store assigned id"), bad)
            .await
            .expect_err("extras come from the catalogue")
            .message,
        "Unknown permission: nope"
    );
}

#[tokio::test]
async fn password_change_requires_the_current_password_and_stores_a_new_hash() {
    let use_case = use_case();
    let created = use_case
        .create(new_user("user@example.com", "Correct#Horse9"))
        .await
        .unwrap();
    let id = created.id.unwrap();

    assert_eq!(
        use_case
            .change_password(id, "wrong".into(), "New#Password999".into())
            .await
            .expect_err("wrong current password is rejected")
            .message,
        "Current password is incorrect"
    );
    assert_eq!(
        use_case
            .change_password(id, "Correct#Horse9".into(), " ".into())
            .await
            .expect_err("new password is required")
            .message,
        "New password is required"
    );
    assert!(
        use_case
            .change_password(id, "Correct#Horse9".into(), "short".into())
            .await
            .is_err(),
        "password policy applies to password changes"
    );

    let changed = use_case
        .change_password(id, "Correct#Horse9".into(), "New#Password999".into())
        .await
        .expect("valid password change succeeds");
    assert!(password::verify("New#Password999", &changed.password));
    assert!(!password::verify("Correct#Horse9", &changed.password));
}

#[tokio::test]
async fn user_lookups_paging_and_tenant_lists_return_only_matching_records() {
    let store = MemoryUserStore::default();
    let use_case = use_case_with_store(store);
    let first = use_case
        .create(new_user("first@example.com", "Correct#Horse9"))
        .await
        .unwrap();
    let mut second_input = new_user("second@example.com", "Correct#Horse9");
    second_input.tenant_id = Some(99);
    let second = use_case.create(second_input).await.unwrap();

    assert_eq!(
        use_case
            .find_by_uuid(first.uuid.clone().unwrap())
            .await
            .unwrap()
            .id,
        first.id
    );
    assert_eq!(
        use_case.find_by_id(999).await.unwrap_err().message,
        "User not found"
    );
    assert!(use_case.find_by_uuid("missing".into()).await.is_err());
    assert_eq!(use_case.find_all().await.unwrap().len(), 2);
    assert_eq!(
        use_case.find_all_by_tenant_id(99).await.unwrap()[0].id,
        second.id
    );
    let (page, total) = use_case.find_page(0, 1, Some("first")).await.unwrap();
    assert_eq!(total, 1);
    assert_eq!(page[0].id, first.id);
}

#[tokio::test]
async fn sysadmin_seed_creates_updates_and_preserves_the_existing_account() {
    let store = MemoryUserStore::default();
    let use_case = use_case_with_store(store);
    unsafe {
        std::env::remove_var("SYSADMIN_EMAIL");
        std::env::remove_var("SYSADMIN_PASSWORD");
    }
    assert_eq!(
        use_case
            .seed_sysadmin(SYSADMIN_PROFILE_ID, 5)
            .await
            .expect_err("email is required")
            .message,
        "SYSADMIN_EMAIL must be set"
    );
    unsafe {
        std::env::set_var("SYSADMIN_EMAIL", "admin@example.com");
    }
    assert_eq!(
        use_case
            .seed_sysadmin(SYSADMIN_PROFILE_ID, 5)
            .await
            .expect_err("password is required")
            .message,
        "SYSADMIN_PASSWORD must be set"
    );
    unsafe {
        std::env::set_var("SYSADMIN_PASSWORD", " ");
    }
    assert!(
        use_case
            .seed_sysadmin(SYSADMIN_PROFILE_ID, 5)
            .await
            .expect_err("empty configuration is rejected")
            .message
            .contains("must not be empty")
    );
    unsafe {
        std::env::set_var("SYSADMIN_EMAIL", "admin@example.com");
        std::env::set_var("SYSADMIN_PASSWORD", "Initial#Password9");
    }

    let created = use_case
        .seed_sysadmin(SYSADMIN_PROFILE_ID, 5)
        .await
        .expect("configured sysadmin is created");
    assert_eq!(created.profile_id, Some(SYSADMIN_PROFILE_ID));
    assert!(created.extra_permissions.is_empty());
    assert_eq!(created.tenant_id, Some(5));
    assert!(password::verify("Initial#Password9", &created.password));
    let original_id = created.id;
    let original_uuid = created.uuid.clone();

    unsafe {
        std::env::set_var("SYSADMIN_EMAIL", "new-admin@example.com");
        std::env::set_var("SYSADMIN_PASSWORD", "Updated#Password10");
    }
    let updated = use_case
        .seed_sysadmin(SYSADMIN_PROFILE_ID, 5)
        .await
        .expect("configured email updates the existing sysadmin");
    assert_eq!(updated.id, original_id);
    assert_eq!(updated.uuid, original_uuid);
    assert_eq!(updated.email, "new-admin@example.com");
    assert!(password::verify("Updated#Password10", &updated.password));

    unsafe {
        std::env::set_var("SYSADMIN_PASSWORD", "Ignored#Password11");
    }
    let unchanged = use_case
        .seed_sysadmin(SYSADMIN_PROFILE_ID, 5)
        .await
        .expect("matching email preserves the existing account");
    assert_eq!(unchanged.password, updated.password);
}
