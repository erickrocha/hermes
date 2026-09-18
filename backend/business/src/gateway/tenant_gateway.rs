use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{fetch_page, Gateway};
use crate::domain::tenant::{Tenant, TenantEntityMapper};
use entity::prelude::TenantEntity as TenantQuery;
use entity::tenant_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{Condition, ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter, QueryOrder};

pub struct TenantGateway {
    db: DbConn,
}

impl TenantGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Gateway<Tenant, tenant_entity::Model, tenant_entity::ActiveModel> for TenantGateway {
    async fn persist(&self, entity: Tenant) -> Result<tenant_entity::ActiveModel, DbErr> {
        let active_model = TenantEntityMapper::build_active_model(entity);
        active_model.save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        TenantQuery::delete_by_id(id)
            .exec(&self.db)
            .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<tenant_entity::Model>, DbErr> {
        TenantQuery::find()
            .filter(tenant_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<tenant_entity::Model>, DbErr> {
        TenantQuery::find()
            .filter(tenant_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<tenant_entity::Model>, DbErr> {
        TenantQuery::find().all(&self.db).await
    }
}

impl TenantGateway {
    /// PD-028.
    pub async fn find_page(&self, page: u64, page_size: u64, search: Option<&str>) -> Result<(Vec<tenant_entity::Model>, u64), DbErr> {
        let mut query = TenantQuery::find().order_by_desc(tenant_entity::Column::Id);
        if let Some(term) = search {
            query = query.filter(
                Condition::any()
                    .add(tenant_entity::Column::BusinessName.contains(term))
                    .add(tenant_entity::Column::CompanyName.contains(term))
                    .add(tenant_entity::Column::TaxId.contains(term))
                    .add(tenant_entity::Column::Email.contains(term)),
            );
        }
        fetch_page(query, &self.db, page, page_size).await
    }
}

