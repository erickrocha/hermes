use sea_orm::prelude::DateTimeUtc;

tokio::task_local! {
    pub static CURRENT_USER: Option<AuditUser>
}

#[derive(Clone,Debug)]
pub struct AuditUser{
    pub id: i64,
    pub email: String,
    pub tenant_id: Option<i64>,
    pub enforce_tenant: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TenantScope {
    Unrestricted,
    Tenant(i64),
    Denied,
}

pub fn tenant_scope() -> TenantScope {
    CURRENT_USER
        .try_with(|user| match user {
            Some(user) if !user.enforce_tenant => TenantScope::Unrestricted,
            Some(user) => user.tenant_id.map(TenantScope::Tenant).unwrap_or(TenantScope::Denied),
            None => TenantScope::Unrestricted,
        })
        .unwrap_or(TenantScope::Unrestricted)
}

/// Runs `fut` with `user` visible to `stamp_audit` via the `CURRENT_USER` task-local.
/// Must wrap the request future in the auth middleware for `before_save` to see it.
pub async fn run_with_user<F: Future>(user: Option<AuditUser>, fut: F) -> F::Output {
    CURRENT_USER.scope(user, fut).await
}

pub trait AuditableActiveModel {
    fn set_uuid(&mut self, v: Vec<u8>);
    fn set_created_at(&mut self, v: DateTimeUtc);
    fn set_updated_at(&mut self, v: DateTimeUtc);
    fn set_created_by(&mut self, v: Option<String>);
    fn set_updated_by(&mut self, v: Option<String>);
}

pub trait TenantActiveModel {
    fn set_tenant_id(&mut self, tenant_id: Option<i64>);
}

pub async fn stamp_audit<T: AuditableActiveModel>(mut am: T, insert: bool) -> T {
    let now = chrono::Utc::now();
    let email = CURRENT_USER
        .try_with(|u| u.as_ref().map(|u| u.email.clone()))
        .ok()
        .flatten();
    if insert {
        am.set_uuid(uuid::Uuid::new_v4().as_bytes().to_vec());
        am.set_created_at(now);
        am.set_created_by(email.clone());
    }
    am.set_updated_at(now);
    am.set_updated_by(email);
    am
}

pub async fn enforce_tenant<T: TenantActiveModel>(mut am: T) -> T {
    match tenant_scope() {
        TenantScope::Tenant(id) => am.set_tenant_id(Some(id)),
        TenantScope::Denied => am.set_tenant_id(None),
        TenantScope::Unrestricted => {}
    }
    am
}

#[macro_export]
macro_rules! impl_auditable_before_save {
    ($active_model:ty) => {
        impl $crate::audit::AuditableActiveModel for $active_model {
            fn set_uuid(&mut self, v: Vec<u8>) {
                self.uuid = sea_orm::Set(v);
            }
            fn set_created_at(&mut self, v: sea_orm::prelude::DateTimeUtc) {
                self.created_at = sea_orm::Set(v);
            }
            fn set_updated_at(&mut self, v: sea_orm::prelude::DateTimeUtc) {
                self.updated_at = sea_orm::Set(v);
            }
            fn set_created_by(&mut self, v: Option<String>) {
                self.created_by = sea_orm::Set(v);
            }
            fn set_updated_by(&mut self, v: Option<String>) {
                self.updated_by = sea_orm::Set(v);
            }
        }

        #[sea_orm::prelude::async_trait::async_trait]
        impl sea_orm::ActiveModelBehavior for $active_model {
            async fn before_save<C>(self, _db: &C, insert: bool) -> Result<Self, sea_orm::DbErr>
            where
                C: sea_orm::ConnectionTrait,
            {
                Ok($crate::audit::stamp_audit(self, insert).await)
            }
        }
    };
}

#[macro_export]
macro_rules! impl_tenant_auditable_before_save {
    ($active_model:ty) => {
        impl $crate::audit::AuditableActiveModel for $active_model {
            fn set_uuid(&mut self, v: Vec<u8>) { self.uuid = sea_orm::Set(v); }
            fn set_created_at(&mut self, v: sea_orm::prelude::DateTimeUtc) { self.created_at = sea_orm::Set(v); }
            fn set_updated_at(&mut self, v: sea_orm::prelude::DateTimeUtc) { self.updated_at = sea_orm::Set(v); }
            fn set_created_by(&mut self, v: Option<String>) { self.created_by = sea_orm::Set(v); }
            fn set_updated_by(&mut self, v: Option<String>) { self.updated_by = sea_orm::Set(v); }
        }

        impl $crate::audit::TenantActiveModel for $active_model {
            fn set_tenant_id(&mut self, tenant_id: Option<i64>) { self.tenant_id = sea_orm::Set(tenant_id); }
        }

        #[sea_orm::prelude::async_trait::async_trait]
        impl sea_orm::ActiveModelBehavior for $active_model {
            async fn before_save<C>(self, _db: &C, insert: bool) -> Result<Self, sea_orm::DbErr>
            where C: sea_orm::ConnectionTrait {
                let model = $crate::audit::enforce_tenant(self).await;
                Ok($crate::audit::stamp_audit(model, insert).await)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{run_with_user, tenant_scope, AuditUser, TenantScope};

    #[tokio::test]
    async fn derives_tenant_scope_from_authenticated_user() {
        let user = AuditUser {
            id: 1,
            email: "owner@example.com".to_string(),
            tenant_id: Some(42),
            enforce_tenant: true,
        };
        let scope = run_with_user(Some(user), async { tenant_scope() }).await;
        assert_eq!(scope, TenantScope::Tenant(42));
    }

    #[tokio::test]
    async fn sysadmin_scope_is_unrestricted() {
        let user = AuditUser {
            id: 1,
            email: "admin@example.com".to_string(),
            tenant_id: None,
            enforce_tenant: false,
        };
        let scope = run_with_user(Some(user), async { tenant_scope() }).await;
        assert_eq!(scope, TenantScope::Unrestricted);
    }
}

