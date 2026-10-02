use business::commons::password;
use business::domain::business_error::BusinessError;
use business::domain::enums::Role;
use business::domain::user::User;
use business::gateway::user_gateway::{UserLookupStore, UserStore};
use business::use_cases::user_use_case::UserUseCase;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryUserStore {
    users: Mutex<Vec<User>>,
}

#[async_trait]
impl UserLookupStore for MemoryUserStore {
    async fn get_by_id(&self, id: i64) -> Result<Option<User>, BusinessError> {
        let users = self
            .users
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(users.iter().find(|user| user.id == Some(id)).cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<User>, BusinessError> {
        let users = self
            .users
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
    async fn save(&self, mut user: User) -> Result<User, BusinessError> {
        let mut users = self
            .users
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
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
            .users
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(users.iter().find(|user| user.email == email).cloned())
    }

    async fn get_by_role(&self, role: Role) -> Result<Option<User>, BusinessError> {
        let users = self
            .users
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(users.iter().find(|user| user.role == role).cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<User>, u64), BusinessError> {
        let users = self
            .users
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
            .users
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(users.clone())
    }

    async fn list_by_tenant(&self, tenant_id: i64) -> Result<Vec<User>, BusinessError> {
        let users = self
            .users
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
        role: Role::TenantUser,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn user_use_case_hashes_passwords_and_reads_from_its_store_port() {
    let use_case = UserUseCase::new(MemoryUserStore::default());
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
    let use_case = UserUseCase::new(MemoryUserStore::default());
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
