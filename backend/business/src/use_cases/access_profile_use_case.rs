use crate::commons::functions::normalize_name;
use crate::domain::access_profile::AccessProfile;
use crate::domain::business_error::BusinessError;
use crate::domain::enums::Role;
use crate::domain::permission::{self, BASELINE, GRANTABLE, OWNER_ONLY};
use crate::domain::user::User;
use crate::gateway::access_profile_gateway::AccessProfileStore;

pub const NAME_REQUIRED: &str = "A profile needs a name";
pub const DUPLICATE_NAME: &str = "This tenant already has a profile with this name";
pub const PROFILE_NOT_FOUND: &str = "Access profile not found";
pub const ROLE_NOT_ELIGIBLE: &str = "Only a tenant user, a driver or a mechanic takes a profile";
pub const UNKNOWN_PERMISSION: &str = "A profile may only hold permissions from the catalogue";
pub const PROFILE_DOES_NOT_FIT: &str = "This profile is not for this user's role, or is inactive";
pub const PROFILE_IN_USE: &str = "A profile in use cannot be deactivated while users hold it";

/// What a caller may do, as the apps and the console consume it (`HRMS-853`).
#[derive(Debug, Clone, PartialEq)]
pub struct EffectivePermissions {
    pub role: Role,
    pub profile_uuid: Option<String>,
    pub profile_name: Option<String>,
    pub permissions: Vec<String>,
}

/// `C-032` (`HRMS-851`/`852`/`853`, `D-27`): the tenant's profiles and who holds which.
pub struct AccessProfileUseCase<S: AccessProfileStore> {
    store: S,
}

impl<S: AccessProfileStore> AccessProfileUseCase<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    pub async fn create(&self, profile: AccessProfile) -> Result<AccessProfile, BusinessError> {
        let profile = self.validated(profile, None).await?;
        self.store.save(profile).await
    }

    pub async fn update(&self, edited: AccessProfile) -> Result<AccessProfile, BusinessError> {
        let current = self.find_by_uuid(edited.uuid.clone().unwrap_or_default()).await?;
        // a profile in use cannot be deactivated or moved to another role under its holders
        if let Some(id) = current.id
            && (!edited.active || edited.base_role != current.base_role)
            && self.store.users_holding(id).await? > 0
        {
            return Err(BusinessError::new(PROFILE_IN_USE.to_string()));
        }
        let profile = self
            .validated(AccessProfile { id: current.id, tenant_id: current.tenant_id, ..edited }, current.id)
            .await?;
        self.store.save(profile).await
    }

    async fn validated(&self, profile: AccessProfile, editing: Option<i64>) -> Result<AccessProfile, BusinessError> {
        let name = profile.name.trim().to_string();
        let name_key = normalize_name(&name);
        if name_key.is_empty() {
            return Err(BusinessError::new(NAME_REQUIRED.to_string()));
        }
        if !permission::takes_profile(&profile.base_role) {
            return Err(BusinessError::new(ROLE_NOT_ELIGIBLE.to_string()));
        }
        let mut permissions: Vec<String> = Vec::new();
        for code in &profile.permissions {
            if permission::find(code).is_none() {
                return Err(BusinessError::new(UNKNOWN_PERMISSION.to_string()));
            }
            if !permissions.contains(code) {
                permissions.push(code.clone());
            }
        }
        permissions.sort();
        let existing = self.store.find_by_key(&name_key, profile.tenant_id).await?;
        if existing.is_some_and(|e| e.id != editing) {
            return Err(BusinessError::new(DUPLICATE_NAME.to_string()));
        }
        Ok(AccessProfile { name, name_key, permissions, ..profile })
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<AccessProfile, BusinessError> {
        self.store
            .get_by_uuid(uuid)
            .await?
            .ok_or_else(|| BusinessError::new(PROFILE_NOT_FOUND.to_string()))
    }

    pub async fn find_page(&self, page: u64, page_size: u64) -> Result<(Vec<AccessProfile>, u64), BusinessError> {
        self.store.list_page(page, page_size).await
    }

    /// Gives `user` the profile, or the role's default when `profile_uuid` is `None`.
    pub async fn assign(&self, user: &User, profile_uuid: Option<String>) -> Result<(), BusinessError> {
        let user_id = user.id.ok_or_else(|| BusinessError::new(PROFILE_NOT_FOUND.to_string()))?;
        if !permission::takes_profile(&user.role) {
            return Err(BusinessError::new(ROLE_NOT_ELIGIBLE.to_string()));
        }
        let profile_id = match profile_uuid {
            None => None,
            Some(uuid) => {
                let profile = self.find_by_uuid(uuid).await?;
                if !profile.active || profile.base_role != user.role || profile.tenant_id != user.tenant_id {
                    return Err(BusinessError::new(PROFILE_DOES_NOT_FIT.to_string()));
                }
                profile.id
            }
        };
        self.store.set_user_profile(user_id, profile_id).await
    }

    /// The caller's permissions: everything for an owner or the platform, otherwise their profile's, or
    /// the built-in default of their role; the baseline always.
    pub async fn effective(&self, user: &User) -> Result<EffectivePermissions, BusinessError> {
        let mut profile_uuid = None;
        let mut profile_name = None;
        let mut codes: Vec<String> = match user.role {
            Role::TenantOwner | Role::SysAdmin => GRANTABLE
                .iter()
                .map(|p| p.code.to_string())
                .chain(OWNER_ONLY.iter().map(|c| c.to_string()))
                .collect(),
            _ => {
                let held = match user.id {
                    Some(id) => self.store.user_profile_id(id).await?,
                    None => None,
                };
                let profile = match held {
                    Some(id) => self.store.get_by_id(id).await?.filter(|p| p.active),
                    None => None,
                };
                match profile {
                    Some(p) => {
                        profile_uuid = p.uuid.clone();
                        profile_name = Some(p.name.clone());
                        p.permissions
                    }
                    None => permission::default_permissions(&user.role).into_iter().map(String::from).collect(),
                }
            }
        };
        codes.extend(BASELINE.iter().map(|c| c.to_string()));
        codes.sort();
        codes.dedup();
        Ok(EffectivePermissions { role: user.role.clone(), profile_uuid, profile_name, permissions: codes })
    }
}
