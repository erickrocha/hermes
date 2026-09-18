use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::city::{City, CityEntityMapper};
use crate::use_cases::reference_import::{find_duplicate_keys, rejected, ImportOutcome, ImportRejection};
use sea_orm::ActiveModelTrait;
use crate::gateway::city_gateway::CityGateway;

pub struct CityUseCase {
    gateway: CityGateway,
}

impl CityUseCase {
    pub fn new(gateway: CityGateway) -> Self {
        Self { gateway }
    }

    pub async fn find_by_id(&self, id: i64) -> Result<City, BusinessError> {
        log::info!("[CityUseCase::find_by_id] Executing for id: {}", id);
        let model = self
            .gateway
            .find_by_id(id)
            .await
            .map_err(|e| {
                let msg = format!("Database error: {}", e);
                log::error!("[CityUseCase::find_by_id] {}", msg);
                BusinessError::new(msg)
            })?;

        match model {
            Some(val) => Ok(CityEntityMapper::from_model(val)),
            None => {
                let msg = format!("City not found with id: {}", id);
                log::error!("[CityUseCase::find_by_id] {}", msg);
                Err(BusinessError::new("City not found".to_string()))
            }
        }
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<City, BusinessError> {
        log::info!("[CityUseCase::find_by_uuid] Executing for uuid: {}", uuid);
        let model = self
            .gateway
            .find_by_uuid(uuid.clone())
            .await
            .map_err(|e| {
                let msg = format!("Database error: {}", e);
                log::error!("[CityUseCase::find_by_uuid] {}", msg);
                BusinessError::new(msg)
            })?;

        match model {
            Some(val) => Ok(CityEntityMapper::from_model(val)),
            None => {
                let msg = format!("City not found with uuid: {}", uuid);
                log::error!("[CityUseCase::find_by_uuid] {}", msg);
                Err(BusinessError::new("City not found".to_string()))
            }
        }
    }

    /// PD-027: mesma validação para o formulário e para a importação.
    pub fn validate(city: &mut City) -> Result<(), String> {
        city.name = city.name.trim().to_string();
        if city.name.is_empty() {
            return Err("name is required".to_string());
        }
        if city.province_id <= 0 {
            return Err("province is required".to_string());
        }
        Ok(())
    }

    pub async fn save(&self, mut city: City) -> Result<City, BusinessError> {
        Self::validate(&mut city).map_err(BusinessError::new)?;

        if let Ok(Some(existing)) = self.gateway.find_by_name(city.province_id, &city.name).await {
            if Some(existing.id) != city.id {
                return Err(BusinessError::new(format!(
                    "City {} already exists in this province",
                    city.name
                )));
            }
        }

        let saved = CityEntityMapper::build_active_model(city)
            .save(self.gateway.db())
            .await
            .map_err(|e| BusinessError::new(format!("Database error: {}", e)))?;
        Ok(CityEntityMapper::from_active_model(saved))
    }

    /// PD-027: tudo ou nada — ver `reference_import`.
    pub async fn import(&self, rows: Vec<City>) -> Result<ImportOutcome, BusinessError> {
        let mut prepared = Vec::with_capacity(rows.len());
        let mut rejections = Vec::new();

        for (index, mut row) in rows.into_iter().enumerate() {
            match Self::validate(&mut row) {
                Ok(()) => prepared.push(row),
                Err(reason) => rejections.push(ImportRejection::new(index, reason)),
            }
        }

        for index in find_duplicate_keys(prepared.iter().map(|c| (c.province_id, c.name.clone()))) {
            rejections.push(ImportRejection::new(index, "duplicate city within the file"));
        }

        if !rejections.is_empty() {
            rejections.sort_by_key(|rejection| rejection.row);
            return Err(rejected(rejections));
        }

        let mut outcome = ImportOutcome::default();
        for mut row in prepared {
            let existing = self
                .gateway
                .find_by_name(row.province_id, &row.name)
                .await
                .map_err(|e| BusinessError::new(format!("Database error: {}", e)))?;
            match existing {
                Some(found) => {
                    row.id = Some(found.id);
                    outcome.updated += 1;
                }
                None => outcome.created += 1,
            }
            CityEntityMapper::build_active_model(row)
                .save(self.gateway.db())
                .await
                .map_err(|e| BusinessError::new(format!("Database error: {}", e)))?;
        }
        Ok(outcome)
    }

    /// PD-028: a lista de cidades é a maior do sistema — é a que mais precisa
    /// disto, e a razão pela qual a regra vale para o projeto inteiro.
    pub async fn find_page(&self, page: u64, page_size: u64, search: Option<&str>) -> Result<(Vec<City>, u64), BusinessError> {
        let (models, total) = self.gateway.find_page(page, page_size, search).await.map_err(|e| {
            let msg = format!("Database error: {}", e);
            log::error!("[CityUseCase::find_page] {}", msg);
            BusinessError::new(msg)
        })?;
        Ok((CityEntityMapper::from_models(models), total))
    }

    pub async fn find_all(&self) -> Result<Vec<City>, BusinessError> {
        log::info!("[CityUseCase::find_all] Executing find_all");
        let models = self
            .gateway
            .find_all()
            .await
            .map_err(|e| {
                let msg = format!("Database error: {}", e);
                log::error!("[CityUseCase::find_all] {}", msg);
                BusinessError::new(msg)
            })?;

        Ok(CityEntityMapper::from_models(models))
    }

    pub async fn find_by_province_id(&self, province_id: i32) -> Result<Vec<City>, BusinessError> {
        log::info!("[CityUseCase::find_by_province_id] Executing for province_id: {}", province_id);
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
        City { id: None, uuid: None, province_id, name: name.to_string() }
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
