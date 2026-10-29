use crate::commons::functions::normalize_name;
use crate::domain::access_profile::AccessProfile;
use crate::domain::authorization::{can_assign_profile_in, can_grant, is_platform_actor, profile_fits_in};
use crate::domain::business_error::BusinessError;
use crate::domain::enums::ProfileScope;
use crate::domain::permission::{self, Grants};
use crate::domain::user::{Access, User};
use crate::gateway::access_profile_gateway::AccessProfileStore;

pub const NAME_REQUIRED: &str = "A profile needs a name";
pub const DUPLICATE_NAME: &str = "This tenant already has a profile with this name";
pub const PROFILE_NOT_FOUND: &str = "Access profile not found";
pub const SYSTEM_PROFILE_READ_ONLY: &str = "A system profile cannot be edited";
pub const SCOPE_NOT_ALLOWED: &str = "A tenant creates tenant profiles; a platform profile belongs to no tenant";
pub const UNKNOWN_PERMISSION: &str = "A profile may only hold permissions from the catalogue, within its scope";
pub const GRANT_EXCEEDS_ACTOR: &str = "Nobody grants a permission they do not hold";
pub const PROFILE_DOES_NOT_FIT: &str = "This profile is not for this user's tenant, or is inactive";
pub const PROFILE_IN_USE: &str = "A profile in use cannot be deactivated while users hold it";

/// What a caller may do, as the apps and the console consume it (`HRMS-853`).
#[derive(Debug, Clone, PartialEq)]
pub struct EffectivePermissions {
    pub scope: ProfileScope,
    pub profile_uuid: Option<String>,
    pub profile_name: Option<String>,
    pub profile_key: Option<String>,
    pub extra_permissions: Vec<String>,
    pub permissions: Vec<String>,
}

impl EffectivePermissions {
    pub fn of(user: &User) -> Self {
        let Access { scope, profile_uuid, profile_name, profile_key, grants, .. } = &user.access;
        Self {
            scope: *scope,
            profile_uuid: profile_uuid.clone(),
            profile_name: profile_name.clone(),
            profile_key: profile_key.clone(),
            extra_permissions: user.extra_permissions.clone(),
            permissions: grants.codes(),
        }
    }
}

/// `C-032`/`C-035` (`HRMS-851`/`852`/`853`, `D-27`): the profiles a tenant sees -- the system ones and
/// its own -- and who holds which, with what extra grants.
pub struct AccessProfileUseCase<S: AccessProfileStore> {
    store: S,
}

impl<S: AccessProfileStore> AccessProfileUseCase<S> {
    pub fn new(store: S) -> Self {
        Self { store }
    }

    /// Creates a profile for `actor`: a tenant actor creates tenant profiles in their own tenant; the
    /// platform creates tenant profiles naming the tenant, or platform profiles naming none.
    pub async fn create(&self, actor: &User, profile: AccessProfile) -> Result<AccessProfile, BusinessError> {
        let profile = AccessProfile { id: None, system: false, key: None, ..profile };
        let profile = self.validated(actor, profile, None).await?;
        self.store.save(profile).await
    }

    pub async fn update(&self, actor: &User, edited: AccessProfile) -> Result<AccessProfile, BusinessError> {
        let current = self.find_by_uuid(edited.uuid.clone().unwrap_or_default()).await?;
        if current.system {
            return Err(BusinessError::new(SYSTEM_PROFILE_READ_ONLY.to_string()));
        }
        // a profile in use cannot be deactivated under its holders
        if let Some(id) = current.id
            && !edited.active
            && self.store.users_holding(id).await? > 0
        {
            return Err(BusinessError::new(PROFILE_IN_USE.to_string()));
        }
        let profile = AccessProfile {
            id: current.id,
            tenant_id: current.tenant_id,
            system: false,
            key: None,
            ..edited
        };
        let profile = self.validated(actor, profile, current.id).await?;
        self.store.save(profile).await
    }

    async fn validated(
        &self,
        actor: &User,
        profile: AccessProfile,
        editing: Option<i64>,
    ) -> Result<AccessProfile, BusinessError> {
        let name = profile.name.trim().to_string();
        let name_key = normalize_name(&name);
        if name_key.is_empty() {
            return Err(BusinessError::new(NAME_REQUIRED.to_string()));
        }
        // A profile of the tenant's own is made inside it; the system administrator may name any tenant.
        if !(profile.tenant_id.is_some() && (is_platform_actor(actor) || profile.tenant_id == actor.tenant_id)) {
            return Err(BusinessError::new(SCOPE_NOT_ALLOWED.to_string()));
        }
        let permissions = permission::normalise_grants(&profile.permissions)
            .map_err(|_| BusinessError::new(UNKNOWN_PERMISSION.to_string()))?;
        if !can_grant(actor, &Grants::from_codes(&permissions)) {
            return Err(BusinessError::new(GRANT_EXCEEDS_ACTOR.to_string()));
        }
        let existing = self.store.find_by_name_key(&name_key, profile.tenant_id).await?;
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

    pub async fn find_by_key(&self, key: &str) -> Result<AccessProfile, BusinessError> {
        self.store
            .find_by_key(key)
            .await?
            .ok_or_else(|| BusinessError::new(PROFILE_NOT_FOUND.to_string()))
    }

    pub async fn find_page(&self, page: u64, page_size: u64) -> Result<(Vec<AccessProfile>, u64), BusinessError> {
        self.store.list_page(page, page_size).await
    }

    /// Resolves the profile `actor` wants to give an account in `target_tenant_id`, refusing one out of
    /// the actor's reach, one that does not fit the tenant, or one whose grants (with `extras`) the actor
    /// does not hold. Returns the profile and the normalised extras.
    pub async fn placeable(
        &self,
        actor: &User,
        profile_uuid: String,
        target_tenant_id: Option<i64>,
        extras: &[String],
    ) -> Result<(AccessProfile, Vec<String>), BusinessError> {
        let profile = self.find_by_uuid(profile_uuid).await?;
        let ancestors = self.ancestors_of(target_tenant_id).await?;
        if can_assign_profile_in(actor, &profile, &ancestors).is_none()
            || !profile_fits_in(&profile, target_tenant_id, &ancestors)
            // `C-036`: a platform profile sits in the platform actor's own tenant, the root, and nowhere else.
            || (crate::domain::authorization::is_system_administrator(&profile) && target_tenant_id != actor.tenant_id)
        {
            return Err(BusinessError::new(PROFILE_DOES_NOT_FIT.to_string()));
        }
        let extras = permission::normalise_grants(extras)
            .map_err(|_| BusinessError::new(UNKNOWN_PERMISSION.to_string()))?;
        // `C-036` (`D-37` 5, `HRMS-876`): nobody grants what they do not hold -- the profile's grants and the extras
        // alike, for every profile. The operational system profiles (tenant user, driver, mechanic) are no longer
        // exempt: whoever registers a driver holds the driver's grants.
        let wanted = Grants::from_codes(profile.permissions.iter().chain(extras.iter()));
        if !can_grant(actor, &wanted) {
            return Err(BusinessError::new(GRANT_EXCEEDS_ACTOR.to_string()));
        }
        Ok((profile, extras))
    }

    /// `C-036`: the tenants above `tenant_id`, nearest first -- a profile one of them made is usable below it.
    pub async fn ancestors_of(&self, tenant_id: Option<i64>) -> Result<Vec<i64>, BusinessError> {
        match tenant_id {
            Some(id) => self.store.ancestors_of(id).await,
            None => Ok(Vec::new()),
        }
    }

    /// Gives `user` the profile and the extra grants (`HRMS-852`).
    pub async fn assign(
        &self,
        actor: &User,
        user: &User,
        profile_uuid: String,
        extras: &[String],
    ) -> Result<(), BusinessError> {
        let user_id = user.id.ok_or_else(|| BusinessError::new(PROFILE_NOT_FOUND.to_string()))?;
        let (profile, extras) = self.placeable(actor, profile_uuid, user.tenant_id, extras).await?;
        let profile_id = profile.id.ok_or_else(|| BusinessError::new(PROFILE_NOT_FOUND.to_string()))?;
        self.store.set_user_access(user_id, profile_id, &extras).await
    }

    /// The caller's permissions, as resolved by the store on read (`HRMS-853`).
    pub fn effective(&self, user: &User) -> EffectivePermissions {
        EffectivePermissions::of(user)
    }

    /// Boot: every system profile exists (the migration seeds the rows) and holds exactly what the
    /// catalogue says for it today. Returns the `SysAdmin` profile's id for the administrator seed.
    pub async fn sync_system_profiles(&self) -> Result<i64, BusinessError> {
        let mut sysadmin_id = None;
        for system in permission::SYSTEM_PROFILES {
            let grants = permission::system_grants(system.key);
            let saved = match self.store.find_by_key(system.key).await? {
                Some(existing) => {
                    if existing.permissions == grants && existing.active {
                        existing
                    } else {
                        self.store
                            .save(AccessProfile { permissions: grants, active: true, ..existing })
                            .await?
                    }
                }
                None => {
                    self.store
                        .save(AccessProfile {
                            id: None,
                            uuid: None,
                            tenant_id: None,
                            name: system.name.to_string(),
                            name_key: normalize_name(system.name),
                            system: true,
                            key: Some(system.key.to_string()),
                            active: true,
                            permissions: grants,
                            created_at: None,
                            created_by: None,
                            updated_at: None,
                            updated_by: None,
                        })
                        .await?
                }
            };
            if system.key == permission::SYS_ADMIN {
                sysadmin_id = saved.id;
            }
        }
        sysadmin_id.ok_or_else(|| BusinessError::new(PROFILE_NOT_FOUND.to_string()))
    }
}
