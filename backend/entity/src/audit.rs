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

/// D-05: deny by default. Sem contexto de usuário — worker, job agendado, um
/// segundo binário, um teste — o escopo é `Denied`, não irrestrito. Acesso a
/// toda a plataforma passou a ser uma concessão explícita (`run_as_platform`)
/// em vez do que se ganha por esquecimento.
pub fn tenant_scope() -> TenantScope {
    CURRENT_USER
        .try_with(|user| match user {
            Some(user) if !user.enforce_tenant => TenantScope::Unrestricted,
            Some(user) => user.tenant_id.map(TenantScope::Tenant).unwrap_or(TenantScope::Denied),
            None => TenantScope::Denied,
        })
        .unwrap_or(TenantScope::Denied)
}

/// O identificador que carimba `created_by`/`updated_by` no que a plataforma
/// escreve fora de qualquer requisição.
pub const PLATFORM_ACTOR: &str = "system";

/// D-05: a concessão explícita. Envolve trabalho que legitimamente roda fora de
/// uma requisição e precisa de alcance de plataforma — seed de boot, migração
/// de dados, e a reescrita de hash legado no login (que acontece antes de
/// existir sessão). Deve envolver o menor trecho possível.
pub async fn run_as_platform<F: Future>(fut: F) -> F::Output {
    let actor = AuditUser {
        id: 0,
        email: PLATFORM_ACTOR.to_string(),
        tenant_id: None,
        enforce_tenant: false,
    };
    run_with_user(Some(actor), fut).await
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

/// D-06: `Denied` recusa a escrita. Antes gravava `tenant_id = NULL` e seguia,
/// produzindo uma linha órfã sem dono — silenciosa, e impossível de atribuir
/// depois. Com D-05 tornando `Denied` alcançável por qualquer código fora de
/// requisição, falhar alto é a única opção defensável.
pub async fn enforce_tenant<T: TenantActiveModel>(mut am: T) -> Result<T, sea_orm::DbErr> {
    match tenant_scope() {
        TenantScope::Tenant(id) => am.set_tenant_id(Some(id)),
        TenantScope::Unrestricted => {}
        TenantScope::Denied => {
            return Err(sea_orm::DbErr::Custom(
                "Refusing to write a tenant-scoped record without a tenant scope. \
                 Wrap deliberate platform-wide work in entity::audit::run_as_platform (D-05/D-06)."
                    .to_string(),
            ));
        }
    }
    Ok(am)
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
                let model = $crate::audit::enforce_tenant(self).await?;
                Ok($crate::audit::stamp_audit(model, insert).await)
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::{enforce_tenant, run_as_platform, run_with_user, tenant_scope, AuditUser, TenantActiveModel, TenantScope};

    /// A minimal `TenantActiveModel` double, so `enforce_tenant`'s effect can
    /// be asserted without a real SeaORM entity or a database.
    struct FakeTenantModel {
        tenant_id: Option<i64>,
    }

    impl TenantActiveModel for FakeTenantModel {
        fn set_tenant_id(&mut self, tenant_id: Option<i64>) {
            self.tenant_id = tenant_id;
        }
    }

    #[tokio::test]
    async fn enforce_tenant_stamps_the_requests_tenant_over_whatever_the_model_carried() {
        let user = AuditUser {
            id: 1,
            email: "owner@example.com".to_string(),
            tenant_id: Some(42),
            enforce_tenant: true,
        };
        let model = FakeTenantModel { tenant_id: Some(999) };
        let stamped = run_with_user(Some(user), enforce_tenant(model)).await.expect("scoped write is allowed");
        assert_eq!(stamped.tenant_id, Some(42));
    }

    #[tokio::test]
    async fn enforce_tenant_refuses_the_write_when_the_caller_is_denied() {
        let user = AuditUser {
            id: 1,
            email: "orphan@example.com".to_string(),
            tenant_id: None,
            enforce_tenant: true,
        };
        let model = FakeTenantModel { tenant_id: Some(999) };
        // D-06: recusa, em vez de gravar uma linha órfã com tenant_id NULL.
        assert!(run_with_user(Some(user), enforce_tenant(model)).await.is_err());
    }

    #[tokio::test]
    async fn enforce_tenant_leaves_an_unrestricted_writers_model_untouched() {
        let user = AuditUser {
            id: 1,
            email: "admin@example.com".to_string(),
            tenant_id: None,
            enforce_tenant: false,
        };
        let model = FakeTenantModel { tenant_id: Some(999) };
        let stamped = run_with_user(Some(user), enforce_tenant(model)).await.expect("unrestricted write is allowed");
        assert_eq!(stamped.tenant_id, Some(999));
    }

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

    #[tokio::test]
    async fn scope_outside_any_request_is_denied_not_unrestricted() {
        // D-05: o padrão histórico era Unrestricted, que dava alcance de
        // plataforma de graça a qualquer worker que alguém viesse a escrever.
        assert_eq!(tenant_scope(), TenantScope::Denied);
        let scope = run_with_user(None, async { tenant_scope() }).await;
        assert_eq!(scope, TenantScope::Denied);
    }

    #[tokio::test]
    async fn run_as_platform_is_the_explicit_grant() {
        let scope = run_as_platform(async { tenant_scope() }).await;
        assert_eq!(scope, TenantScope::Unrestricted);

        let model = FakeTenantModel { tenant_id: Some(999) };
        let stamped = run_as_platform(enforce_tenant(model)).await.expect("platform write is allowed");
        assert_eq!(stamped.tenant_id, Some(999));
    }
}
