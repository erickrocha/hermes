use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{fetch_page, Gateway};
use crate::domain::city::{City, CityEntityMapper};
use entity::city_entity;
use entity::province_entity;
use entity::prelude::CityEntity as CityQuery;
use entity::prelude::ProvinceEntity as ProvinceQuery;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{ActiveModelTrait, ColumnTrait, DbConn, DbErr, DeleteResult, EntityTrait, QueryFilter, QueryOrder};
use std::collections::HashSet;

pub struct CityGateway {
    db: DbConn,
}

impl CityGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &DbConn {
        &self.db
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

impl CityGateway {
    /// PD-028.
    pub async fn find_page(&self, page: u64, page_size: u64, search: Option<&str>) -> Result<(Vec<city_entity::Model>, u64), DbErr> {
        let mut query = CityQuery::find().order_by_asc(city_entity::Column::Name);
        if let Some(term) = search {
            query = query.filter(city_entity::Column::Name.contains(term));
        }
        fetch_page(query, &self.db, page, page_size).await
    }
}

impl CityGateway {
    /// Chave de negócio da importação: nome dentro da província.
    pub async fn find_by_name(
        &self,
        province_id: i64,
        name: &str,
    ) -> Result<Option<city_entity::Model>, DbErr> {
        CityQuery::find()
            .filter(city_entity::Column::ProvinceId.eq(province_id))
            .filter(city_entity::Column::Name.eq(name))
            .one(&self.db)
            .await
    }

    /// DEF-RD-01/03: which of these provinces actually exist. An unknown
    /// `provinceId` used to be discovered by the database, mid-import, as a
    /// foreign-key violation — after earlier rows had already been written,
    /// and phrased in terms of a constraint name. Asking first turns it into
    /// an ordinary validation result for a named row.
    pub async fn existing_province_ids(&self, ids: &[i64]) -> Result<HashSet<i64>, DbErr> {
        if ids.is_empty() {
            return Ok(HashSet::new());
        }
        Ok(ProvinceQuery::find()
            .filter(province_entity::Column::Id.is_in(ids.to_vec()))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|province| province.id)
            .collect())
    }
}
