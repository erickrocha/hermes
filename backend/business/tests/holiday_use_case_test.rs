use business::domain::business_error::BusinessError;
use business::domain::holiday::Holiday;
use business::gateway::holiday_gateway::{HolidayStore, HolidayStoreError};
use business::use_cases::holiday_use_case::{DUPLICATE_HOLIDAY, HolidayUseCase};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryHolidayStore {
    holidays: Mutex<Vec<Holiday>>,
}

#[async_trait]
impl HolidayStore for MemoryHolidayStore {
    async fn save(&self, mut holiday: Holiday) -> Result<Holiday, HolidayStoreError> {
        let mut holidays = self.holidays.lock().map_err(|_| {
            HolidayStoreError::Storage(BusinessError::new("memory store poisoned".into()))
        })?;
        if holidays.iter().any(|existing| {
            existing.tenant_id == holiday.tenant_id
                && existing.date == holiday.date
                && existing.id != holiday.id
        }) {
            return Err(HolidayStoreError::DuplicateDate);
        }
        let id = holiday.id.unwrap_or(holidays.len() as i64 + 1);
        holiday.id = Some(id);
        holiday.uuid = Some(format!("holiday-{id}"));
        holidays.push(holiday.clone());
        Ok(holiday)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<Holiday>, BusinessError> {
        let holidays = self
            .holidays
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(holidays
            .iter()
            .find(|holiday| holiday.id == Some(id))
            .cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<Holiday>, BusinessError> {
        let holidays = self
            .holidays
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(holidays
            .iter()
            .find(|holiday| holiday.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn delete_by_id(&self, id: i64) -> Result<(), BusinessError> {
        let mut holidays = self
            .holidays
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        holidays.retain(|holiday| holiday.id != Some(id));
        Ok(())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<Holiday>, u64), BusinessError> {
        let holidays = self
            .holidays
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let total = holidays.len() as u64;
        let start = (page * page_size) as usize;
        Ok((
            holidays
                .iter()
                .skip(start)
                .take(page_size as usize)
                .cloned()
                .collect(),
            total,
        ))
    }
}

fn new_holiday(name: &str) -> Holiday {
    Holiday {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        date: chrono::NaiveDate::from_ymd_opt(2026, 10, 12).expect("valid test date"),
        name: name.into(),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn holiday_use_case_uses_its_store_port_without_a_database_gateway() {
    let use_case = HolidayUseCase::new(MemoryHolidayStore::default());

    let created = use_case
        .create(new_holiday("  Republic Day  "))
        .await
        .expect("holiday is stored");
    let found = use_case
        .find_by_uuid(created.uuid.clone().expect("store assigned uuid"))
        .await
        .expect("holiday is found");
    let (page, total) = use_case.find_page(0, 10).await.expect("page is listed");

    assert_eq!(created.name, "Republic Day");
    assert_eq!(found.id, created.id);
    assert_eq!(total, 1);
    assert_eq!(page.len(), 1);
}

#[tokio::test]
async fn holiday_use_case_maps_a_duplicate_date_to_the_domain_error() {
    let use_case = HolidayUseCase::new(MemoryHolidayStore::default());
    use_case
        .create(new_holiday("First name"))
        .await
        .expect("first holiday is stored");

    let error = use_case
        .create(new_holiday("Different name"))
        .await
        .expect_err("same tenant and date conflict");

    assert_eq!(error.message, DUPLICATE_HOLIDAY);
}

#[tokio::test]
async fn holiday_use_case_removes_a_found_holiday_by_its_persisted_id() {
    let use_case = HolidayUseCase::new(MemoryHolidayStore::default());
    let holiday = use_case
        .create(new_holiday("Republic Day"))
        .await
        .expect("holiday is stored");

    use_case
        .remove(holiday.uuid.expect("store assigned uuid"))
        .await
        .expect("holiday is removed");
    let (_, total) = use_case.find_page(0, 10).await.expect("page is listed");

    assert_eq!(total, 0);
}
