use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::Gateway;
use crate::domain::city::{City, CityEntityMapper};
use entity::city_entity;
use entity::prelude::CityEntity as CityQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter, QueryOrder};

pub struct CityGateway {
    db: DbConn,
}

impl CityGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    pub async fn find_by_province_id(&self, province_id: i32) -> Result<Vec<city_entity::Model>, DbErr> {
        CityQuery::find()
            .filter(city_entity::Column::ProvinceId.eq(province_id))
            .order_by_asc(city_entity::Column::Name)
            .all(&self.db)
            .await
    }
}

#[async_trait]
impl Gateway<City, city_entity::Model, city_entity::ActiveModel> for CityGateway {
    async fn persist(&self, entity: City) -> Result<city_entity::ActiveModel, DbErr> {
        let active_model = CityEntityMapper::build_active_model(entity);
        active_model.save(&self.db).await
    }

    async fn delete_by_id(&self, id: i64) -> Result<DeleteResult, DbErr> {
        CityQuery::delete_by_id(id)
            .exec(&self.db)
            .await
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<city_entity::Model>, DbErr> {
        CityQuery::find()
            .filter(city_entity::Column::Id.eq(id))
            .one(&self.db)
            .await
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<city_entity::Model>, DbErr> {
        CityQuery::find()
            .filter(city_entity::Column::Uuid.eq(string_to_bytes(&uuid)))
            .one(&self.db)
            .await
    }

    async fn find_all(&self) -> Result<Vec<city_entity::Model>, DbErr> {
        CityQuery::find()
            .order_by_asc(city_entity::Column::Name)
            .all(&self.db)
            .await
    }
}
