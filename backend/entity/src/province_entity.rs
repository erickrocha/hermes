//! `SeaORM` Entity

use sea_orm::Set;
use sea_orm::entity::prelude::*;
use sea_orm::prelude::async_trait::async_trait;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "province")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub acronym: String,
    pub name: String,
    pub country_code: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::city_entity::Entity")]
    City,
}

impl Related<super::city_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::City.def()
    }
}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert && self.uuid.is_not_set() {
            self.uuid = Set(Uuid::new_v4().as_bytes().to_vec());
        }
        Ok(self)
    }
}
