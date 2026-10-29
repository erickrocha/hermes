use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::access_profile::AccessProfile;
use crate::domain::enums::ProfileScope;
use crate::domain::permission::{GrantScope, Grants};
use chrono::NaiveDateTime;
use entity::user_entity::{ActiveModel, Model};
use sea_orm::{NotSet, Set};

/// `C-036`: an account reaches every tenant it names -- the legacy platform reach -- only through the system
/// administrator's profile, placed in the root tenant. Nothing else about a profile is a "scope"; any other
/// account acts in its own tenant, or in a descendant through `X-Tenant-Uuid` and the `tenant.child.*` grants.
fn reach_of(profile: &AccessProfile) -> ProfileScope {
    if profile.system && profile.key.as_deref() == Some(crate::domain::permission::SYS_ADMIN) {
        ProfileScope::Platform
    } else {
        ProfileScope::Tenant
    }
}

/// `C-035`: what a user may do, resolved from their profile and their own extra grants. Never persisted:
/// `UserGateway` fills it on every read, so a change to the profile or the extras takes effect on the
/// holder's next request (`HRMS-853`). A `User` built without a store (tests, a payload) carries the
/// default -- a tenant-scoped account holding only the baseline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Access {
    pub scope: ProfileScope,
    pub profile_uuid: Option<String>,
    pub profile_name: Option<String>,
    /// The system profile's key (`SysAdmin`, `Driver`, ...) when the user holds one; `None` for a tenant-made profile.
    pub profile_key: Option<String>,
    pub grants: Grants,
    /// `C-036`: the caller's own tenant while a request acts inside a descendant (`X-Tenant-Uuid`), where `tenant_id`
    /// is the descendant's; `None` otherwise. Moving a tenant in the tree is judged from here.
    pub acting_from: Option<i64>,
}

impl Default for Access {
    fn default() -> Self {
        Self {
            scope: ProfileScope::Tenant,
            profile_uuid: None,
            profile_name: None,
            profile_key: None,
            grants: Grants::effective(&[], &[]),
            acting_from: None,
        }
    }
}

impl Access {
    /// The profile's grants plus the user's extras plus the baseline; an inactive profile grants nothing
    /// beyond the baseline and the extras.
    pub fn resolve(profile: &AccessProfile, extras: &[String]) -> Self {
        let held: &[String] = if profile.active { &profile.permissions } else { &[] };
        Self {
            scope: reach_of(profile),
            profile_uuid: profile.uuid.clone(),
            profile_name: Some(profile.name.clone()),
            profile_key: profile.key.clone(),
            grants: Grants::effective(held, extras),
            acting_from: None,
        }
    }

    /// The access a holder of the system profile `key` has, straight from the catalogue -- for tests,
    /// in-memory stores and anything else with no profile row at hand. `None` for an unknown key.
    pub fn of_system_profile(key: &str) -> Option<Self> {
        let system = crate::domain::permission::system_profile(key)?;
        let scope = if system.key == crate::domain::permission::SYS_ADMIN { ProfileScope::Platform } else { ProfileScope::Tenant };
        Some(Self {
            profile_key: Some(system.key.to_string()),
            profile_name: Some(system.name.to_string()),
            ..Self::from_grants(scope, crate::domain::permission::system_grants(system.key))
        })
    }

    /// Builds an access from grants alone (tests and in-memory stores).
    pub fn from_grants<I, S>(scope: ProfileScope, grants: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let codes: Vec<String> = grants.into_iter().map(|g| g.as_ref().to_string()).collect();
        Self { scope, profile_uuid: None, profile_name: None, profile_key: None, grants: Grants::effective(&codes, &[]), acting_from: None }
    }
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub email: String,
    pub name: Option<String>,
    pub password: String,
    pub enabled: bool,
    pub tenant_id: Option<i64>,
    /// The access profile held; `None` only on a payload before the profile is resolved and set.
    pub profile_id: Option<i64>,
    /// `C-035`: grants the user holds on top of their profile's. Validated against the catalogue when saved.
    pub extra_permissions: Vec<String>,
    pub cpf: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
    /// Resolved on read, never stored.
    pub access: Access,
}

impl User {
    /// The tenant a request acts in: the platform actor names one (`requested`, `None` when it did not), everyone
    /// else acts in their own. `C-036` gave the platform actor the root tenant, so `self.tenant_id.or(requested)`
    /// would silently ignore the tenant it named.
    pub fn target_tenant(&self, requested: Option<i64>) -> Option<i64> {
        if self.access.scope == ProfileScope::Platform { requested } else { self.tenant_id }
    }

    /// The tenant a request is confined to: the account's own, except for the platform actor, who is not
    /// confined to one (it reaches every tenant it names). Before `C-036` this was just `tenant_id`, which was
    /// `None` for the platform actor; the root tenant it now belongs to must not read as a confinement.
    pub fn bound_tenant(&self) -> Option<i64> {
        if self.access.scope == ProfileScope::Platform { None } else { self.tenant_id }
    }

    pub fn allows(&self, code: &str) -> bool {
        self.access.grants.allows(code)
    }

    pub fn allows_fully(&self, code: &str) -> bool {
        self.access.grants.allows_fully(code)
    }

    /// Holds `code` over own records only.
    pub fn only_own(&self, code: &str) -> bool {
        self.access.grants.only_own(code)
    }

    pub fn grant_scope(&self, code: &str) -> Option<GrantScope> {
        self.access.grants.scope(code)
    }

    pub fn is_platform(&self) -> bool {
        self.access.scope == ProfileScope::Platform
    }
}

fn parse_permissions(text: &str) -> Vec<String> {
    serde_json::from_str(text).unwrap_or_default()
}

pub struct UserEntityMapper {}
impl EntityMapper<User, Model, ActiveModel> for UserEntityMapper {
    fn build_active_model(d: User) -> ActiveModel {
        ActiveModel {
            id: match d.id {
                Some(id) => Set(id),
                None => NotSet,
            },
            uuid: match d.uuid {
                Some(uuid) => Set(string_to_bytes(&uuid)),
                None => NotSet,
            },
            name: Set(d.name.to_owned()),
            email: Set(d.email.to_owned()),
            password: Set(d.password.to_owned()),
            enabled: Set(d.enabled.to_owned()),
            // Owned by the billing worker; a profile update must not clear it.
            blocked_reason: NotSet,
            tenant_id: Set(d.tenant_id),
            profile_id: match d.profile_id {
                Some(id) => Set(id),
                None => NotSet,
            },
            extra_permissions: Set(serde_json::to_string(&d.extra_permissions).unwrap_or_else(|_| "[]".into())),
            cpf: Set(d.cpf),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: Default::default(),
        }
    }

    fn from_model(e: Model) -> User {
        User {
            id: Some(e.id),
            uuid: Some(bytes_para_string(e.uuid)),
            name: e.name,
            email: e.email,
            password: e.password,
            enabled: e.enabled,
            tenant_id: e.tenant_id,
            profile_id: Some(e.profile_id),
            extra_permissions: parse_permissions(&e.extra_permissions),
            cpf: e.cpf,
            created_at: Some(e.created_at.naive_utc()),
            created_by: e.created_by,
            updated_at: Some(e.updated_at.naive_utc()),
            updated_by: e.updated_by,
            access: Access::default(),
        }
    }

    fn from_active_model(mut e: ActiveModel) -> User {
        use sea_orm::TryIntoModel;
        let model: Result<Model, _> = e.clone().try_into_model();
        match model {
            Ok(m) => Self::from_model(m),
            Err(_) => User {
                id: e.id.take(),
                uuid: e.uuid.take().map(bytes_para_string),
                name: e.name.take().flatten(),
                email: e.email.take().unwrap_or_default(),
                password: e.password.take().unwrap_or_default(),
                enabled: e.enabled.take().unwrap_or(true),
                tenant_id: e.tenant_id.take().flatten(),
                profile_id: e.profile_id.take(),
                extra_permissions: e.extra_permissions.take().map(|p| parse_permissions(&p)).unwrap_or_default(),
                cpf: e.cpf.take().flatten(),
                created_at: e.created_at.take().map(|dt| dt.naive_utc()),
                created_by: e.created_by.take().flatten(),
                updated_at: e.updated_at.take().map(|dt| dt.naive_utc()),
                updated_by: e.updated_by.take().flatten(),
                access: Access::default(),
            },
        }
    }
}
