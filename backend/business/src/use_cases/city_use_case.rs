use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::city::{City, CityEntityMapper};
use crate::gateway::city_gateway::CityGateway;
use crate::use_cases::reference_import::{
    ImportOutcome, ImportRejection, ReferenceDataError, RejectionReason, find_duplicate_keys,
    fold_key,
};
use sea_orm::{ActiveModelTrait, TransactionTrait};

/// `city.name` is `varchar(255)` (migration `m20260916_000007`).
const NAME_MAX: usize = 255;

pub struct CityUseCase {
    gateway: CityGateway,
}

impl CityUseCase {
    pub fn new(gateway: CityGateway) -> Self {
        Self { gateway }
    }

    pub async fn find_by_id(&self, id: i64) -> Result<City, BusinessError> {
        log::info!("[CityUseCase::find_by_id] Executing for id: {}", id);
        let model = self.gateway.find_by_id(id).await.map_err(|e| {
            let msg = format!("Database error: {}", e);
            log::error!("[CityUseCase::find_by_id] {}", msg);
            BusinessError::new(msg)
        })?;

        match model {
            Some(val) => Ok(CityEntityMapper::from_model(val)),
            None => {
                let msg = format!("City not found with id: {}", id);
                log::error!("[CityUseCase::find_by_id] {}", msg);
                Err(BusinessError::not_found("City not found".to_string()))
            }
        }
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<City, BusinessError> {
        log::info!("[CityUseCase::find_by_uuid] Executing for uuid: {}", uuid);
        let model = self.gateway.find_by_uuid(uuid.clone()).await.map_err(|e| {
            let msg = format!("Database error: {}", e);
            log::error!("[CityUseCase::find_by_uuid] {}", msg);
            BusinessError::new(msg)
        })?;

        match model {
            Some(val) => Ok(CityEntityMapper::from_model(val)),
            None => {
                let msg = format!("City not found with uuid: {}", uuid);
                log::error!("[CityUseCase::find_by_uuid] {}", msg);
                Err(BusinessError::not_found("City not found".to_string()))
            }
        }
    }

    /// PD-027: mesma validação para o formulário e para a importação.
    pub fn validate(city: &mut City) -> Result<(), RejectionReason> {
        city.name = city.name.trim().to_string();
        if city.name.is_empty() {
            return Err(RejectionReason::NameRequired);
        }
        // DEF-RD-03: `city.name` is `varchar(255)`. Checked here, the operator
        // is told the limit; left to the database, the same row came back as
        // "Data too long for column 'name' at row 1" — a row number that is
        // the statement's, not the file's.
        if city.name.chars().count() > NAME_MAX {
            return Err(RejectionReason::NameTooLong);
        }
        if city.province_id <= 0 {
            return Err(RejectionReason::ProvinceRequired);
        }
        Ok(())
    }

    pub async fn save(&self, mut city: City) -> Result<City, ReferenceDataError> {
        Self::validate(&mut city).map_err(|reason| ReferenceDataError::one(0, reason))?;

        // DEF-RD-03: an unknown province is a validation failure the operator
        // can act on, not a foreign-key violation quoting the schema.
        let known = self
            .gateway
            .existing_province_ids(&[city.province_id])
            .await
            .map_err(|e| ReferenceDataError::unavailable("CityUseCase::save", e))?;
        if !known.contains(&city.province_id) {
            return Err(ReferenceDataError::one(
                0,
                RejectionReason::ProvinceNotFound,
            ));
        }

        if let Ok(Some(existing)) = self
            .gateway
            .find_by_name(city.province_id, &city.name)
            .await
            && Some(existing.id) != city.id
        {
            return Err(ReferenceDataError::one(
                0,
                RejectionReason::CityAlreadyExists,
            ));
        }

        let saved = CityEntityMapper::build_active_model(city)
            .save(self.gateway.db())
            .await
            .map_err(|e| ReferenceDataError::unavailable("CityUseCase::save", e))?;
        Ok(CityEntityMapper::from_active_model(saved))
    }

    /// PD-027: tudo ou nada — ver `reference_import`.
    pub async fn import(&self, rows: Vec<City>) -> Result<ImportOutcome, ReferenceDataError> {
        let mut prepared = Vec::with_capacity(rows.len());
        let mut rejections = Vec::new();

        for (index, mut row) in rows.into_iter().enumerate() {
            match Self::validate(&mut row) {
                Ok(()) => prepared.push((index, row)),
                Err(reason) => rejections.push(ImportRejection::new(index, reason)),
            }
        }

        // Positions are reported against the rows as the operator sent them;
        // `prepared` holds only the valid ones, so each keeps its original index.
        // DEF-RD-02: the key is folded, because the storage this check protects
        // does not distinguish case or accents either.
        let keys = prepared
            .iter()
            .map(|(_, c)| (c.province_id, fold_key(&c.name)));
        for position in find_duplicate_keys(keys) {
            rejections.push(ImportRejection::new(
                prepared[position].0,
                RejectionReason::DuplicateCityInFile,
            ));
        }

        // DEF-RD-01/03: every province the file names is checked before
        // anything is written. This is the case the defect was found with — an
        // IBGE file with one wrong province id — and it used to surface as a
        // foreign-key error after the earlier rows had gone in.
        //
        // Only worth a round trip once the rows stand on their own: a batch
        // already rejected is not going to be written either way.
        if rejections.is_empty() {
            let referenced: Vec<i64> = prepared.iter().map(|(_, c)| c.province_id).collect();
            let known = self
                .gateway
                .existing_province_ids(&referenced)
                .await
                .map_err(|e| ReferenceDataError::unavailable("CityUseCase::import", e))?;
            for (index, row) in &prepared {
                if !known.contains(&row.province_id) {
                    rejections.push(ImportRejection::new(
                        *index,
                        RejectionReason::ProvinceNotFound,
                    ));
                }
            }
        }

        if !rejections.is_empty() {
            rejections.sort_by_key(|rejection| rejection.row);
            return Err(ReferenceDataError::Rejected(rejections));
        }

        // PD-027 all-or-nothing. Validation alone cannot promise it: a write
        // can still fail (a lost connection, a column limit nobody modelled),
        // and without a transaction the rows before it stayed (DEF-RD-01).
        let transaction = self
            .gateway
            .db()
            .begin()
            .await
            .map_err(|e| ReferenceDataError::unavailable("CityUseCase::import", e))?;

        let mut outcome = ImportOutcome::default();
        for (_, mut row) in prepared {
            let existing = self
                .gateway
                .find_by_name(row.province_id, &row.name)
                .await
                .map_err(|e| ReferenceDataError::unavailable("CityUseCase::import", e))?;
            match existing {
                Some(found) => {
                    row.id = Some(found.id);
                    outcome.updated += 1;
                }
                None => outcome.created += 1,
            }
            if let Err(e) = CityEntityMapper::build_active_model(row)
                .save(&transaction)
                .await
            {
                let _ = transaction.rollback().await;
                return Err(ReferenceDataError::unavailable("CityUseCase::import", e));
            }
        }

        transaction
            .commit()
            .await
            .map_err(|e| ReferenceDataError::unavailable("CityUseCase::import", e))?;
        Ok(outcome)
    }

    /// PD-028: a lista de cidades é a maior do sistema — é a que mais precisa
    /// disto, e a razão pela qual a regra vale para o projeto inteiro.
    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<City>, u64), BusinessError> {
        let (models, total) = self
            .gateway
            .find_page(page, page_size, search)
            .await
            .map_err(|e| {
                let msg = format!("Database error: {}", e);
                log::error!("[CityUseCase::find_page] {}", msg);
                BusinessError::new(msg)
            })?;
        Ok((CityEntityMapper::from_models(models), total))
    }

    pub async fn find_all(&self) -> Result<Vec<City>, BusinessError> {
        log::info!("[CityUseCase::find_all] Executing find_all");
        let models = self.gateway.find_all().await.map_err(|e| {
            let msg = format!("Database error: {}", e);
            log::error!("[CityUseCase::find_all] {}", msg);
            BusinessError::new(msg)
        })?;

        Ok(CityEntityMapper::from_models(models))
    }

    pub async fn find_by_province_id(&self, province_id: i32) -> Result<Vec<City>, BusinessError> {
        log::info!(
            "[CityUseCase::find_by_province_id] Executing for province_id: {}",
            province_id
        );
        let models = self
            .gateway
            .find_by_province_id(province_id)
            .await
            .map_err(|e| {
                let msg = format!("Database error: {}", e);
                log::error!("[CityUseCase::find_by_province_id] {}", msg);
                BusinessError::new(msg)
            })?;

        Ok(CityEntityMapper::from_models(models))
    }
}

#[cfg(test)]
mod validation_tests {
    use super::CityUseCase;
    use crate::domain::city::City;

    fn city(name: &str, province_id: i64) -> City {
        City {
            id: None,
            uuid: None,
            province_id,
            name: name.to_string(),
        }
    }

    #[test]
    fn trims_the_name() {
        let mut row = city("  Campinas ", 1);
        CityUseCase::validate(&mut row).expect("valid row");
        assert_eq!(row.name, "Campinas");
    }

    #[test]
    fn rejects_a_blank_name_or_a_missing_province() {
        let mut blank = city("   ", 1);
        assert!(CityUseCase::validate(&mut blank).is_err());

        let mut orphan = city("Campinas", 0);
        assert!(CityUseCase::validate(&mut orphan).is_err());
    }
}
