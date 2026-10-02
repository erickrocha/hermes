use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::string_to_bytes;
use crate::commons::gateway::{Gateway, fetch_page};
use crate::domain::business_error::BusinessError;
use crate::domain::province::{Province, ProvinceEntityMapper};
use entity::prelude::ProvinceEntity as ProvinceQuery;
use entity::province_entity;
use sea_orm::prelude::async_trait::async_trait;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DbConn, DbErr, DeleteResult, EntityTrait,
    QueryFilter, QueryOrder, QuerySelect, TransactionTrait,
};

#[async_trait]
pub trait ProvinceStore: Send + Sync {
    async fn save(&self, province: Province) -> Result<Province, BusinessError>;
    async fn import(&self, provinces: Vec<Province>) -> Result<(u64, u64), BusinessError>;
    async fn find_by_id(&self, id: i64) -> Result<Option<Province>, BusinessError>;
    async fn find_by_uuid(&self, uuid: String) -> Result<Option<Province>, BusinessError>;
    async fn find_by_country_code(
        &self,
        country_code: &str,
    ) -> Result<Vec<Province>, BusinessError>;
    async fn find_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<Province>, u64), BusinessError>;
    async fn find_by_acronym(
        &self,
        country_code: &str,
        acronym: &str,
    ) -> Result<Option<Province>, BusinessError>;
    async fn distinct_country_codes(&self) -> Result<Vec<String>, BusinessError>;
}

pub struct ProvinceGateway {
    db: DbConn,
}

impl ProvinceGateway {
    pub fn new(db: DbConn) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &DbConn {
        &self.db
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

    /// DEF-RD-08 (PD-022/PD-027): the countries the platform can actually
    /// place a tenant in — those that have provinces, however they got them.
    /// Derived rather than listed, so importing a new country's provinces is
    /// by itself enough to make it selectable; that is the point of PD-027.
    pub async fn distinct_country_codes(&self) -> Result<Vec<String>, DbErr> {
        ProvinceQuery::find()
            .select_only()
            .column(province_entity::Column::CountryCode)
            .distinct()
            .order_by_asc(province_entity::Column::CountryCode)
            .into_tuple::<String>()
            .all(&self.db)
            .await
    }
}

#[async_trait]
impl ProvinceStore for ProvinceGateway {
    async fn save(&self, province: Province) -> Result<Province, BusinessError> {
        ProvinceEntityMapper::build_active_model(province)
            .save(&self.db)
            .await
            .map(ProvinceEntityMapper::from_active_model)
            .map_err(database_error)
    }

    async fn import(&self, provinces: Vec<Province>) -> Result<(u64, u64), BusinessError> {
        let transaction = self.db.begin().await.map_err(database_error)?;
        let mut created = 0_u64;
        let mut updated = 0_u64;

        for mut province in provinces {
            let existing = ProvinceQuery::find()
                .filter(province_entity::Column::CountryCode.eq(&province.country_code))
                .filter(province_entity::Column::Acronym.eq(&province.acronym))
                .one(&transaction)
                .await;
            let existing = match existing {
                Ok(existing) => existing,
                Err(error) => {
                    let _ = transaction.rollback().await;
                    return Err(database_error(error));
                }
            };
            match existing {
                Some(found) => {
                    province.id = Some(found.id);
                    updated += 1;
                }
                None => created += 1,
            }
            if let Err(error) = ProvinceEntityMapper::build_active_model(province)
                .save(&transaction)
                .await
            {
                let _ = transaction.rollback().await;
                return Err(database_error(error));
            }
        }

        transaction.commit().await.map_err(database_error)?;
        Ok((created, updated))
    }

    async fn find_by_id(&self, id: i64) -> Result<Option<Province>, BusinessError> {
        <Self as Gateway<
            Province,
            province_entity::Model,
            province_entity::ActiveModel,
        >>::find_by_id(self, id)
        .await
        .map(|model| model.map(ProvinceEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn find_by_uuid(&self, uuid: String) -> Result<Option<Province>, BusinessError> {
        <Self as Gateway<
            Province,
            province_entity::Model,
            province_entity::ActiveModel,
        >>::find_by_uuid(self, uuid)
        .await
        .map(|model| model.map(ProvinceEntityMapper::from_model))
        .map_err(database_error)
    }

    async fn find_by_country_code(
        &self,
        country_code: &str,
    ) -> Result<Vec<Province>, BusinessError> {
        ProvinceGateway::find_by_country_code(self, country_code)
            .await
            .map(ProvinceEntityMapper::from_models)
            .map_err(database_error)
    }

    async fn find_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<Province>, u64), BusinessError> {
        ProvinceGateway::find_page(self, page, page_size, search)
            .await
            .map(|(models, total)| (ProvinceEntityMapper::from_models(models), total))
            .map_err(database_error)
    }

    async fn find_by_acronym(
        &self,
        country_code: &str,
        acronym: &str,
    ) -> Result<Option<Province>, BusinessError> {
        ProvinceGateway::find_by_acronym(self, country_code, acronym)
            .await
            .map(|model| model.map(ProvinceEntityMapper::from_model))
            .map_err(database_error)
    }

    async fn distinct_country_codes(&self) -> Result<Vec<String>, BusinessError> {
        ProvinceGateway::distinct_country_codes(self)
            .await
            .map_err(database_error)
    }
}

fn database_error(error: DbErr) -> BusinessError {
    let message = format!("Database error: {error}");
    log::error!("[ProvinceGateway] {message}");
    BusinessError::new(message)
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

impl ProvinceGateway {
    /// PD-028.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<province_entity::Model>, u64), DbErr> {
        let mut query = ProvinceQuery::find().order_by_asc(province_entity::Column::Acronym);
        if let Some(term) = search {
            query = query.filter(
                Condition::any()
                    .add(province_entity::Column::Name.contains(term))
                    .add(province_entity::Column::Acronym.contains(term)),
            );
        }
        fetch_page(query, &self.db, page, page_size).await
    }
}

impl ProvinceGateway {
    /// Chave de negócio da importação: sigla dentro do país. É por ela que uma
    /// reimportação atualiza a linha existente em vez de duplicá-la (regra 9 do
    /// operacao-trm, aprendida lá: nunca duplicar em importação automática).
    pub async fn find_by_acronym(
        &self,
        country_code: &str,
        acronym: &str,
    ) -> Result<Option<province_entity::Model>, DbErr> {
        ProvinceQuery::find()
            .filter(province_entity::Column::CountryCode.eq(country_code))
            .filter(province_entity::Column::Acronym.eq(acronym))
            .one(&self.db)
            .await
    }
}
