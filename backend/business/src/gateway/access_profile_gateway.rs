use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete};
use crate::domain::access_profile::{AccessProfile, AccessProfileEntityMapper};
use crate::domain::business_error::BusinessError;
use entity::access_profile_entity;
use entity::audit;
use entity::prelude::{AccessProfileEntity as AccessProfileQuery, UserEntity as UserQuery};
use entity::user_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DbConn, DbErr, DeleteResult, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Select,
};

/// `C-035`: the profiles a scope sees -- its own tenant's and the system ones (no tenant). The platform
/// sees every profile; a denied scope none. The `tenant_select` shape with the system rows let through,
/// written out here because no other tenant-owned table shares rows across tenants.
fn profile_select(query: Select<AccessProfileQuery>) -> Select<AccessProfileQuery> {
    match audit::tenant_scope() {
        audit::TenantScope::Unrestricted => query,
        audit::TenantScope::Tenant(id) => query.filter(
            Condition::any()
                .add(access_profile_entity::Column::TenantId.eq(id))
                .add(access_profile_entity::Column::TenantId.is_null()),
        ),
        audit::TenantScope::Denied => query.filter(Expr::cust("1 = 0")),
    }
}

#[async_trait]
pub trait AccessProfileStore: Send + Sync {
    async fn save(&self, profile: AccessProfile) -> Result<AccessProfile, BusinessError>;
    async fn get_by_id(&self, id: i64) -> Result<Option<AccessProfile>, BusinessError>;
    async fn get_by_uuid(&self, uuid: String) -> Result<Option<AccessProfile>, BusinessError>;
    /// A profile by normalised name inside `tenant_id` -- or among the system profiles when `None`.
    async fn find_by_name_key(&self, name_key: &str, tenant_id: Option<i64>) -> Result<Option<AccessProfile>, BusinessError>;
    /// A system profile by its key (`SysAdmin`, `Driver`, ...).
    async fn find_by_key(&self, key: &str) -> Result<Option<AccessProfile>, BusinessError>;
    /// The profiles the caller's scope sees: the system ones and the tenant's own.
    async fn list_page(&self, page: u64, page_size: u64) -> Result<(Vec<AccessProfile>, u64), BusinessError>;
    /// Sets a user's profile and their extra grants. Touches those two columns.
    async fn set_user_access(&self, user_id: i64, profile_id: i64, extras: &[String]) -> Result<(), BusinessError>;
    async fn users_holding(&self, profile_id: i64) -> Result<u64, BusinessError>;
}

pub struct AccessProfileGateway {
    db: DbConn,
}

impl AccessProfileGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Access profile database error: {error}");
    log::error!("[AccessProfileGateway] {message}");
    BusinessError::new(message)
}


#[async_trait]
impl AccessProfileStore for AccessProfileGateway {
    async fn save(&self, profile: AccessProfile) -> Result<AccessProfile, BusinessError> {
        let active = <Self as Gateway<AccessProfile, access_profile_entity::Model, access_profile_entity::ActiveModel>>::persist(self, profile)
            .await
            .map_err(database_error)?;
        Ok(AccessProfileEntityMapper::from_active_model(active))
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<AccessProfile>, BusinessError> {
        let model = <Self as Gateway<AccessProfile, access_profile_entity::Model, access_profile_entity::ActiveModel>>::find_by_id(self, id)
            .await
            .map_err(database_error)?;
        Ok(model.map(AccessProfileEntityMapper::from_model))
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<AccessProfile>, BusinessError> {
        let model = <Self as Gateway<AccessProfile, access_profile_entity::Model, access_profile_entity::ActiveModel>>::find_by_uuid(self, uuid)
            .await
            .map_err(database_error)?;
        Ok(model.map(AccessProfileEntityMapper::from_model))
    }

    async fn find_by_name_key(&self, name_key: &str, tenant_id: Option<i64>) -> Result<Option<AccessProfile>, BusinessError> {
        let tenant = match tenant_id {
            Some(id) => access_profile_entity::Column::TenantId.eq(id),
            None => access_profile_entity::Column::TenantId.is_null(),
        };
        let model = profile_select(AccessProfileQuery::find())
            .filter(access_profile_entity::Column::NameKey.eq(name_key))
            .filter(tenant)
            .one(&self.db)
            .await
            .map_err(database_error)?;
        Ok(model.map(AccessProfileEntityMapper::from_model))
    }

    async fn find_by_key(&self, key: &str) -> Result<Option<AccessProfile>, BusinessError> {
        let model = profile_select(AccessProfileQuery::find())
            .filter(access_profile_entity::Column::Key.eq(key))
            .one(&self.db)
            .await
            .map_err(database_error)?;
        Ok(model.map(AccessProfileEntityMapper::from_model))
    }

    async fn list_page(&self, page: u64, page_size: u64) -> Result<(Vec<AccessProfile>, u64), BusinessError> {
        let query = profile_select(AccessProfileQuery::find())
            .order_by_desc(access_profile_entity::Column::System)
            .order_by_asc(access_profile_entity::Column::Name);
        let (models, total) = fetch_page(query, &self.db, page, page_size).await.map_err(database_error)?;
        Ok((AccessProfileEntityMapper::from_models(models), total))
    }

    async fn set_user_access(&self, user_id: i64, profile_id: i64, extras: &[String]) -> Result<(), BusinessError> {
        let extras = serde_json::to_string(extras).unwrap_or_else(|_| "[]".into());
        UserQuery::update_many()
            .col_expr(user_entity::Column::ProfileId, Expr::value(profile_id))
            .col_expr(user_entity::Column::ExtraPermissions, Expr::value(extras))
            .filter(user_entity::Column::Id.eq(user_id))
            .exec(&self.db)
            .await
            .map(|_| ())
            .map_err(database_error)
    }

    async fn users_holding(&self, profile_id: i64) -> Result<u64, BusinessError> {
        UserQuery::find()
            .filter(user_entity::Column::ProfileId.eq(profile_id))
            .count(&self.db)
            .await
            .map_err(database_error)
    }
}

#[async_trait]
impl Gateway<AccessProfile, access_profile_entity::Model, access_profile_entity::ActiveModel> for AccessProfileGateway {
    async fn persist(&self, entity: AccessProfile) -> Result<access_profile_entity::ActiveModel, DbErr> {
        AccessProfileEntityMapper::build_active_model(entity).save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        tenant_delete(
            AccessProfileQuery::delete_many().filter(access_profile_entity::Column::Id.eq(id)),
            access_profile_entity::Column::TenantId,
        )
        .exec(&self.db)
        .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<access_profile_entity::Model>, DbErr> {
        profile_select(AccessProfileQuery::find())
            .filter(access_profile_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<access_profile_entity::Model>, DbErr> {
        profile_select(AccessProfileQuery::find())
            .filter(access_profile_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<access_profile_entity::Model>, DbErr> {
        profile_select(AccessProfileQuery::find()).all(&self.db).await
    }
}
