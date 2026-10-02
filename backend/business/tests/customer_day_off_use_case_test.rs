use business::domain::business_error::BusinessError;
use business::domain::customer::Customer;
use business::domain::customer_day_off::CustomerDayOff;
use business::gateway::customer_day_off_gateway::{CustomerDayOffStore, CustomerDayOffStoreError};
use business::use_cases::customer_day_off_use_case::{
    CustomerDayOffUseCase, DAY_OFF_NOT_FOUND, DUPLICATE_DAY_OFF,
};
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryCustomerDayOffStore {
    day_offs: Mutex<Vec<CustomerDayOff>>,
}

#[async_trait]
impl CustomerDayOffStore for MemoryCustomerDayOffStore {
    async fn save(
        &self,
        mut day_off: CustomerDayOff,
    ) -> Result<CustomerDayOff, CustomerDayOffStoreError> {
        let mut day_offs = self.day_offs.lock().map_err(|_| {
            CustomerDayOffStoreError::Storage(BusinessError::new("memory store poisoned".into()))
        })?;
        if day_offs.iter().any(|existing| {
            existing.customer_id == day_off.customer_id
                && existing.date == day_off.date
                && existing.id != day_off.id
        }) {
            return Err(CustomerDayOffStoreError::DuplicateDate);
        }
        let id = day_off.id.unwrap_or(day_offs.len() as i64 + 1);
        day_off.id = Some(id);
        day_off.uuid = Some(format!("customer-day-off-{id}"));
        day_offs.push(day_off.clone());
        Ok(day_off)
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<CustomerDayOff>, BusinessError> {
        let day_offs = self
            .day_offs
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(day_offs
            .iter()
            .find(|day_off| day_off.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn delete_by_id(&self, id: i64) -> Result<(), BusinessError> {
        let mut day_offs = self
            .day_offs
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        day_offs.retain(|day_off| day_off.id != Some(id));
        Ok(())
    }

    async fn list_page_by_customer(
        &self,
        customer_id: i64,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<CustomerDayOff>, u64), BusinessError> {
        let day_offs = self
            .day_offs
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let filtered: Vec<_> = day_offs
            .iter()
            .filter(|day_off| day_off.customer_id == customer_id)
            .cloned()
            .collect();
        let total = filtered.len() as u64;
        let start = (page * page_size) as usize;
        Ok((
            filtered
                .into_iter()
                .skip(start)
                .take(page_size as usize)
                .collect(),
            total,
        ))
    }
}

fn customer(id: Option<i64>) -> Customer {
    Customer {
        id,
        uuid: Some("customer-uuid".into()),
        tenant_id: Some(42),
        name: "Customer".into(),
        status: "Active".into(),
        notes: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn day_off(customer_id: i64, day: u32) -> CustomerDayOff {
    CustomerDayOff {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        customer_id,
        date: chrono::NaiveDate::from_ymd_opt(2026, 10, day).expect("valid test date"),
        reason: Some("Closed".into()),
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn customer_day_off_use_case_scopes_history_and_removal_to_the_customer() {
    let use_case = CustomerDayOffUseCase::new(MemoryCustomerDayOffStore::default());
    let first = use_case
        .add(
            &customer(Some(7)),
            day_off(7, 12).date,
            Some("Closed".into()),
        )
        .await
        .expect("day off is added");

    let (history, total) = use_case
        .history(&customer(Some(7)), 0, 10)
        .await
        .expect("customer history is listed");
    let other_customer = use_case
        .remove(
            &customer(Some(8)),
            first.uuid.clone().expect("store assigned uuid"),
        )
        .await
        .expect_err("another customer's day off cannot be removed");

    assert_eq!(total, 1);
    assert_eq!(history[0].customer_id, 7);
    assert_eq!(other_customer.message, DAY_OFF_NOT_FOUND);
}

#[tokio::test]
async fn customer_day_off_use_case_maps_duplicate_date_to_the_domain_error() {
    let use_case = CustomerDayOffUseCase::new(MemoryCustomerDayOffStore::default());
    use_case
        .add(
            &customer(Some(7)),
            chrono::NaiveDate::from_ymd_opt(2026, 10, 12).unwrap(),
            None,
        )
        .await
        .expect("first day off is added");

    let error = use_case
        .add(
            &customer(Some(7)),
            chrono::NaiveDate::from_ymd_opt(2026, 10, 12).unwrap(),
            None,
        )
        .await
        .expect_err("duplicate customer date is rejected");

    assert_eq!(error.message, DUPLICATE_DAY_OFF);
}

#[tokio::test]
async fn customer_day_off_use_case_refuses_a_customer_without_a_persisted_id() {
    let use_case = CustomerDayOffUseCase::new(MemoryCustomerDayOffStore::default());

    let error = use_case
        .add(
            &customer(None),
            chrono::NaiveDate::from_ymd_opt(2026, 10, 12).unwrap(),
            None,
        )
        .await
        .expect_err("customer id is required");

    assert_eq!(error.message, "Customer identity is incomplete");
}
