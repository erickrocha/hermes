use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page, tenant_delete, tenant_select};
use crate::domain::user::{User, UserEntityMapper};
use entity::prelude::UserEntity as UserQuery;
use entity::user_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DbConn, DbErr, DeleteResult, EntityTrait,
    QueryFilter, QueryOrder,
};

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

    pub async fn find_by_role(
        db: &DbConn,
        role: String,
    ) -> Result<Option<user_entity::Model>, DbErr> {
        UserQuery::find()
            .filter(user_entity::Column::Role.eq(role))
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
