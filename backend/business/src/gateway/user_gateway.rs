use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::access_profile::{AccessProfile, AccessProfileEntityMapper};
use crate::domain::business_error::BusinessError;
use crate::domain::user::{Access, User, UserEntityMapper};
use entity::access_profile_entity;
use entity::prelude::{AccessProfileEntity as AccessProfileQuery, UserEntity as UserQuery};
use entity::user_entity;
use std::collections::HashMap;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DbConn, DbErr, DeleteResult, EntityTrait,
    QueryFilter, QueryOrder,
};

#[async_trait]
pub trait UserLookupStore: Send + Sync {
    async fn get_by_id(&self, id: i64) -> Result<Option<User>, BusinessError>;
    async fn get_by_uuid(&self, uuid: String) -> Result<Option<User>, BusinessError>;
}

#[async_trait]
pub trait UserStore: UserLookupStore {
    /// The authentication-time lookup by immutable id. Authentication runs
    /// before any tenant scope exists, so an implementation backed by the
    /// database must not go through `tenant_select` (see
    /// `UserGateway::find_by_user_id`); `get_by_id` would answer "not found"
    /// under D-05's deny-by-default scope. The default suits in-memory stores,
    /// which have no scope.
    async fn get_for_authentication(&self, id: i64) -> Result<Option<User>, BusinessError> {
        self.get_by_id(id).await
    }
    async fn save(&self, user: User) -> Result<User, BusinessError>;
    async fn get_by_email(&self, email: String) -> Result<Option<User>, BusinessError>;
    /// Every account holding this CPF (digits only). More than one means the CPF
    /// is shared across tenants, which login treats as ambiguous. The default
    /// suits in-memory stores that do not carry CPFs.
    async fn get_by_cpf(&self, _cpf: String) -> Result<Vec<User>, BusinessError> {
        Ok(Vec::new())
    }
    /// The one account holding the system profile `key` (`SysAdmin` at boot). `None` by default for
    /// in-memory stores.
    async fn get_by_profile_key(&self, _key: &str) -> Result<Option<User>, BusinessError> {
        Ok(None)
    }
    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<User>, u64), BusinessError>;
    async fn list_all(&self) -> Result<Vec<User>, BusinessError>;
    async fn list_by_tenant(&self, tenant_id: i64) -> Result<Vec<User>, BusinessError>;
}

pub struct UserGateway {
    db: DbConn,
}

impl UserGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &DbConn {
        &self.db
    }
}

#[async_trait]
impl UserLookupStore for UserGateway {
    async fn get_by_id(&self, id: i64) -> Result<Option<User>, BusinessError> {
        let model = <Self as Gateway<User, user_entity::Model, user_entity::ActiveModel>>::find_by_id(self, id)
            .await
            .map_err(lookup_database_error)?;
        self.resolve_one(model.map(UserEntityMapper::from_model)).await
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<User>, BusinessError> {
        let model = <Self as Gateway<User, user_entity::Model, user_entity::ActiveModel>>::find_by_uuid(self, uuid)
            .await
            .map_err(lookup_database_error)?;
        self.resolve_one(model.map(UserEntityMapper::from_model)).await
    }
}

impl UserGateway {
    /// `C-035`: every user read from the store carries their resolved [`Access`]. The profile is read by
    /// primary key and outside any tenant scope: it is the user's own, already reached through the user
    /// row, and authentication resolves it before a scope exists.
    async fn resolve_one(&self, user: Option<User>) -> Result<Option<User>, BusinessError> {
        match user {
            None => Ok(None),
            Some(user) => Ok(self.resolve_all(vec![user]).await?.pop()),
        }
    }

    async fn resolve_all(&self, users: Vec<User>) -> Result<Vec<User>, BusinessError> {
        let mut ids: Vec<i64> = users.iter().filter_map(|u| u.profile_id).collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.is_empty() {
            return Ok(users);
        }
        let profiles: HashMap<i64, AccessProfile> = AccessProfileQuery::find()
            .filter(access_profile_entity::Column::Id.is_in(ids))
            .all(&self.db)
            .await
            .map_err(lookup_database_error)?
            .into_iter()
            .map(|m| (m.id, AccessProfileEntityMapper::from_model(m)))
            .collect();
        Ok(users
            .into_iter()
            .map(|mut user| {
                if let Some(profile) = user.profile_id.and_then(|id| profiles.get(&id)) {
                    user.access = Access::resolve(profile, &user.extra_permissions);
                }
                user
            })
            .collect())
    }
}

#[async_trait]
impl UserStore for UserGateway {
    async fn get_for_authentication(&self, id: i64) -> Result<Option<User>, BusinessError> {
        let model = Self::find_by_user_id(&self.db, id)
            .await
            .map_err(lookup_database_error)?;
        self.resolve_one(model.map(UserEntityMapper::from_model)).await
    }

    async fn save(&self, user: User) -> Result<User, BusinessError> {
        let active_model = <Self as Gateway<User, user_entity::Model, user_entity::ActiveModel>>::persist(self, user)
            .await
            .map_err(database_error)?;
        let saved = self.resolve_one(Some(UserEntityMapper::from_active_model(active_model))).await?;
        saved.ok_or_else(|| BusinessError::new("User was not saved".to_string()))
    }

    async fn get_by_email(&self, email: String) -> Result<Option<User>, BusinessError> {
        let model = UserGateway::find_by_email(&self.db, email)
            .await
            .map_err(database_error)?;
        self.resolve_one(model.map(UserEntityMapper::from_model)).await
    }

    async fn get_by_cpf(&self, cpf: String) -> Result<Vec<User>, BusinessError> {
        let models = UserGateway::find_all_by_cpf(&self.db, cpf)
            .await
            .map_err(database_error)?;
        self.resolve_all(models.into_iter().map(UserEntityMapper::from_model).collect()).await
    }

    async fn get_by_profile_key(&self, key: &str) -> Result<Option<User>, BusinessError> {
        let model = UserGateway::find_by_profile_key(&self.db, key)
            .await
            .map_err(database_error)?;
        self.resolve_one(model.map(UserEntityMapper::from_model)).await
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<User>, u64), BusinessError> {
        let (models, total) = UserGateway::find_page(self, page, page_size, search)
            .await
            .map_err(database_error)?;
        Ok((self.resolve_all(UserEntityMapper::from_models(models)).await?, total))
    }

    async fn list_all(&self) -> Result<Vec<User>, BusinessError> {
        let models = <Self as Gateway<User, user_entity::Model, user_entity::ActiveModel>>::find_all(self)
            .await
            .map_err(database_error)?;
        self.resolve_all(UserEntityMapper::from_models(models)).await
    }

    async fn list_by_tenant(&self, tenant_id: i64) -> Result<Vec<User>, BusinessError> {
        let models = UserGateway::find_all_by_tenant_id(self, tenant_id)
            .await
            .map_err(database_error)?;
        self.resolve_all(UserEntityMapper::from_models(models)).await
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("User database error: {error}");
    log::error!("[UserGateway] {message}");
    BusinessError::new(message)
}

fn lookup_database_error(error: DbErr) -> BusinessError {
    let message = format!("User lookup database error: {error}");
    log::error!("[UserGateway] {message}");
    BusinessError::new(message)
}

#[async_trait]
impl Gateway<User, user_entity::Model, user_entity::ActiveModel> for UserGateway {
    async fn persist(&self, entity: User) -> Result<user_entity::ActiveModel, DbErr> {
        let active_model = UserEntityMapper::build_active_model(entity);
        active_model.save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            UserQuery::delete_many().filter(user_entity::Column::Id.eq(id)),
            user_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<user_entity::Model>, DbErr> {
        tenant_select(UserQuery::find(), user_entity::Column::TenantId)
            .filter(user_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<user_entity::Model>, DbErr> {
        tenant_select(UserQuery::find(), user_entity::Column::TenantId)
            .filter(user_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<user_entity::Model>, DbErr> {
        tenant_select(UserQuery::find(), user_entity::Column::TenantId)
            .all(&self.db)
            .await
    }
}

impl UserGateway {
    /// PD-028. Continua passando por `tenant_select`: paginar não pode ser um
    /// caminho de leitura que escapa do escopo de tenant.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<user_entity::Model>, u64), DbErr> {
        let mut query = tenant_select(UserQuery::find(), user_entity::Column::TenantId)
            .order_by_desc(user_entity::Column::Id);
        if let Some(term) = search {
            query = query.filter(
                Condition::any()
                    .add(user_entity::Column::Name.contains(term))
                    .add(user_entity::Column::Email.contains(term)),
            );
        }
        fetch_page(query, &self.db, page, page_size).await
    }
}

impl UserGateway {
    pub async fn find_all_by_cpf(
        db: &DbConn,
        cpf: String,
    ) -> Result<Vec<user_entity::Model>, DbErr> {
        UserQuery::find()
            .filter(user_entity::Column::Cpf.eq(cpf))
            .all(db)
            .await
    }

    pub async fn find_by_email(
        db: &DbConn,
        email: String,
    ) -> Result<Option<user_entity::Model>, DbErr> {
        UserQuery::find()
            .filter(user_entity::Column::Email.eq(email))
            .one(db)
            .await
    }

    /// DEF-IA-01/DEF-IA-02: the authentication-time lookup, keyed on the
    /// account's immutable id rather than on its address.
    ///
    /// Deliberately *not* the `Gateway::find_by_id` above: that one goes
    /// through `tenant_select`, and authentication happens before any tenant
    /// scope exists. It is also deliberately not `find_by_email`: an address
    /// is editable and reusable, so a token resolved by address follows the
    /// address to whoever holds it next.
    pub async fn find_by_user_id(
        db: &DbConn,
        id: i64,
    ) -> Result<Option<user_entity::Model>, DbErr> {
        UserQuery::find()
            .filter(user_entity::Column::Id.eq(id))
            .one(db)
            .await
    }

    /// The account holding the system profile `key`; unscoped, like every boot-time lookup.
    pub async fn find_by_profile_key(
        db: &DbConn,
        key: &str,
    ) -> Result<Option<user_entity::Model>, DbErr> {
        let Some(profile) = AccessProfileQuery::find()
            .filter(access_profile_entity::Column::Key.eq(key))
            .one(db)
            .await?
        else {
            return Ok(None);
        };
        UserQuery::find()
            .filter(user_entity::Column::ProfileId.eq(profile.id))
            .one(db)
            .await
    }

    pub async fn find_all_by_tenant_id(
        &self,
        tenant_id: i64,
    ) -> Result<Vec<user_entity::Model>, DbErr> {
        UserQuery::find()
            .filter(user_entity::Column::TenantId.eq(tenant_id))
            .all(&self.db)
            .await
    }
}
