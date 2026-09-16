use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{tenant_delete, tenant_select, Gateway};
use crate::domain::user::{User, UserEntityMapper};
use entity::prelude::UserEntity as UserQuery;
use entity::user_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter};

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
        tenant_select(UserQuery::find(), user_entity::Column::TenantId).all(&self.db).await
    }
}

impl UserGateway {
    pub async fn find_by_email(db: &DbConn, email: String) -> Result<Option<user_entity::Model>, DbErr> {
        UserQuery::find()
            .filter(user_entity::Column::Email.eq(email))
            .one(db)
            .await
    }

    pub async fn find_by_role(db: &DbConn, role: String) -> Result<Option<user_entity::Model>, DbErr> {
        UserQuery::find()
            .filter(user_entity::Column::Role.eq(role))
            .one(db)
            .await
    }

    pub async fn find_all_by_tenant_id(&self, tenant_id: i64) -> Result<Vec<user_entity::Model>, DbErr> {
        UserQuery::find()
            .filter(user_entity::Column::TenantId.eq(tenant_id))
            .all(&self.db)
            .await
    }
}
