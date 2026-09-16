//! `SeaORM` Entity

use sea_orm::entity::prelude::*;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::Set;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "city")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub province_id: i64,
    pub name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::province_entity::Entity",
        from = "Column::ProvinceId",
        to = "super::province_entity::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Province,
}

impl Related<super::province_entity::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Province.def()
    }
}

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert
            && self.uuid.is_not_set() {
                self.uuid = Set(Uuid::new_v4().as_bytes().to_vec());
            }
        Ok(self)
    }
}
