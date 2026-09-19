use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::province::{normalize_country_code, Province, ProvinceEntityMapper};
use crate::use_cases::reference_import::{
    find_duplicate_keys, fold_key, ImportOutcome, ImportRejection, ReferenceDataError,
    RejectionReason,
};
use sea_orm::{ActiveModelTrait, TransactionTrait};
use crate::gateway::province_gateway::ProvinceGateway;

/// `province.name` is `varchar(255)` and `province.acronym` `varchar(10)`
/// (migration `m20260916_000006`).
const NAME_MAX: usize = 255;
const ACRONYM_MAX: usize = 10;

pub struct ProvinceUseCase {
    gateway: ProvinceGateway,
}

impl ProvinceUseCase {
    pub fn new(gateway: ProvinceGateway) -> Self {
        Self { gateway }
    }

    /// PD-027: valida e normaliza uma província antes de gravar. A mesma
    /// função serve o formulário e a importação, para que uma linha importada
    /// não possa entrar em um estado que o formulário recusaria.
    pub fn validate(province: &mut Province) -> Result<(), RejectionReason> {
        province.name = province.name.trim().to_string();
        province.acronym = province.acronym.trim().to_ascii_uppercase();
        if province.name.is_empty() {
            return Err(RejectionReason::NameRequired);
        }
        // DEF-RD-03: the column limits, stated as rules rather than left to
        // the database to report as a truncation error.
        if province.name.chars().count() > NAME_MAX {
            return Err(RejectionReason::NameTooLong);
        }
        if province.acronym.is_empty() {
            return Err(RejectionReason::AcronymRequired);
        }
        if province.acronym.chars().count() > ACRONYM_MAX {
            return Err(RejectionReason::AcronymTooLong);
        }
        match normalize_country_code(&province.country_code) {
            Some(code) => province.country_code = code,
            None => return Err(RejectionReason::CountryCodeInvalid),
        }
        Ok(())
    }

    pub async fn save(&self, mut province: Province) -> Result<Province, ReferenceDataError> {
        Self::validate(&mut province).map_err(|reason| ReferenceDataError::one(0, reason))?;

        // A sigla é única dentro do país: gravar uma que já existe em outra
        // linha seria criar uma segunda verdade para o mesmo lugar.
        if let Ok(Some(existing)) = self
            .gateway
            .find_by_acronym(&province.country_code, &province.acronym)
            .await
            && Some(existing.id) != province.id
        {
            return Err(ReferenceDataError::one(0, RejectionReason::ProvinceAlreadyExists));
        }

        let saved = ProvinceEntityMapper::build_active_model(province)
            .save(self.gateway.db())
            .await
            .map_err(|e| ReferenceDataError::unavailable("ProvinceUseCase::save", e))?;
        Ok(ProvinceEntityMapper::from_active_model(saved))
    }

    /// PD-027: tudo ou nada — ver `reference_import`.
    pub async fn import(&self, rows: Vec<Province>) -> Result<ImportOutcome, ReferenceDataError> {
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
        // DEF-RD-02: folded, so "SP"/"sp" collide here exactly as they do in
        // the unique index this check stands in front of.
        let keys = prepared
            .iter()
            .map(|(_, p)| (p.country_code.clone(), fold_key(&p.acronym)));
        for position in find_duplicate_keys(keys) {
            rejections.push(ImportRejection::new(
                prepared[position].0,
                RejectionReason::DuplicateAcronymInFile,
            ));
        }

        if !rejections.is_empty() {
            rejections.sort_by_key(|rejection| rejection.row);
            return Err(ReferenceDataError::Rejected(rejections));
        }

        // PD-027 all-or-nothing (DEF-RD-01): a failure partway through the
        // loop used to leave the rows before it committed.
        let transaction = self
            .gateway
            .db()
            .begin()
            .await
            .map_err(|e| ReferenceDataError::unavailable("ProvinceUseCase::import", e))?;

        let mut outcome = ImportOutcome::default();
        for (_, mut row) in prepared {
            let existing = self
                .gateway
                .find_by_acronym(&row.country_code, &row.acronym)
                .await
                .map_err(|e| ReferenceDataError::unavailable("ProvinceUseCase::import", e))?;
            match existing {
                Some(found) => {
                    row.id = Some(found.id);
                    outcome.updated += 1;
                }
                None => outcome.created += 1,
            }
            if let Err(e) = ProvinceEntityMapper::build_active_model(row).save(&transaction).await {
                let _ = transaction.rollback().await;
                return Err(ReferenceDataError::unavailable("ProvinceUseCase::import", e));
            }
        }

        transaction
            .commit()
            .await
            .map_err(|e| ReferenceDataError::unavailable("ProvinceUseCase::import", e))?;
        Ok(outcome)
    }

    /// PD-028.
    pub async fn find_page(&self, page: u64, page_size: u64, search: Option<&str>) -> Result<(Vec<Province>, u64), BusinessError> {
        let (models, total) = self.gateway.find_page(page, page_size, search).await.map_err(|e| {
            let msg = format!("Database error: {}", e);
            log::error!("[ProvinceUseCase::find_page] {}", msg);
            BusinessError::new(msg)
        })?;
        Ok((ProvinceEntityMapper::from_models(models), total))
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Province, BusinessError> {
        log::info!("[ProvinceUseCase::find_by_id] Executing for id: {}", id);
        let model = self.gateway.find_by_id(id).await.map_err(|e| {
            let msg = format!("Database error: {}", e);
            log::error!("[ProvinceUseCase::find_by_id] {}", msg);
            BusinessError::new(msg)
        })?;

        match model {
            Some(val) => Ok(ProvinceEntityMapper::from_model(val)),
            None => {
                let msg = format!("Province not found with id: {}", id);
                log::error!("[ProvinceUseCase::find_by_id] {}", msg);
                Err(BusinessError::not_found("Province not found".to_string()))
            }
        }
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<Province, BusinessError> {
        log::info!("[ProvinceUseCase::find_by_uuid] Executing for uuid: {}",uuid);
        let model = self.gateway.find_by_uuid(uuid.clone()).await.map_err(|e| {
            let msg = format!("Database error: {}", e);
            log::error!("[ProvinceUseCase::find_by_uuid] {}", msg);
            BusinessError::new(msg)
        })?;

        match model {
            Some(val) => Ok(ProvinceEntityMapper::from_model(val)),
            None => {
                let msg = format!("Province not found with uuid: {}", uuid);
                log::error!("[ProvinceUseCase::find_by_uuid] {}", msg);
                Err(BusinessError::not_found("Province not found".to_string()))
            }
        }
    }

    pub async fn find_by_country_code(&self,country_code: String) -> Result<Vec<Province>, BusinessError> {
        log::info!("[ProvinceUseCase::find_by_country_code] Executing for country_code: {}",country_code);
        let models = self
            .gateway
            .find_by_country_code(&country_code)
            .await
            .map_err(|e| {
                let msg = format!("Database error: {}", e);
                log::error!("[ProvinceUseCase::find_by_country_code] {}", msg);
                BusinessError::new(msg)
            })?;

        Ok(ProvinceEntityMapper::from_models(models))
    }
}

#[cfg(test)]
mod validation_tests {
    use super::ProvinceUseCase;
    use crate::domain::province::Province;

    fn province(acronym: &str, name: &str, country: &str) -> Province {
        Province {
            id: None,
            uuid: None,
            acronym: acronym.to_string(),
            name: name.to_string(),
            country_code: country.to_string(),
        }
    }

    #[test]
    fn trims_and_upper_cases_what_a_csv_typically_carries() {
        let mut row = province(" sp ", "  São Paulo  ", " br ");
        ProvinceUseCase::validate(&mut row).expect("valid row");
        assert_eq!(row.acronym, "SP");
        assert_eq!(row.name, "São Paulo");
        assert_eq!(row.country_code, "BR");
    }

    #[test]
    fn rejects_blank_fields_and_bad_country_codes() {
        let mut blank_name = province("SP", "   ", "BR");
        assert!(ProvinceUseCase::validate(&mut blank_name).is_err());

        let mut blank_acronym = province("  ", "São Paulo", "BR");
        assert!(ProvinceUseCase::validate(&mut blank_acronym).is_err());

        let mut bad_country = province("SP", "São Paulo", "BRA");
        assert!(ProvinceUseCase::validate(&mut bad_country).is_err());
    }
}
