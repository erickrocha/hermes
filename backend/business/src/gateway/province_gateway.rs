use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::Gateway;
use crate::domain::province::{Province, ProvinceEntityMapper};
use entity::prelude::ProvinceEntity as ProvinceQuery;
use entity::province_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter,
    QueryOrder,
};

pub struct ProvinceGateway {
    db: DbConn,
}

impl ProvinceGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    pub async fn find_by_country_code(
        &self,
        country_code: &str,
    ) -> Result<Vec<province_entity::Model>, DbErr> {
        ProvinceQuery::find()
            .filter(province_entity::Column::CountryCode.eq(country_code))
            .order_by_asc(province_entity::Column::Acronym)
            .all(&self.db)
            .await
    }
}

#[async_trait]
impl Gateway<Province, province_entity::Model, province_entity::ActiveModel> for ProvinceGateway {
    async fn persist(&self, entity: Province) -> Result<province_entity::ActiveModel, DbErr> {
        let active_model = ProvinceEntityMapper::build_active_model(entity);
        active_model.save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        ProvinceQuery::delete_by_id(id).exec(&self.db).await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<province_entity::Model>, DbErr> {
        ProvinceQuery::find()
            .filter(province_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<province_entity::Model>, DbErr> {
        ProvinceQuery::find()
            .filter(province_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<province_entity::Model>, DbErr> {
        ProvinceQuery::find()
            .order_by_asc(province_entity::Column::Acronym)
            .all(&self.db)
            .await
    }
}
