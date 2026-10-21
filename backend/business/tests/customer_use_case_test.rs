use business::domain::business_error::BusinessError;
use business::domain::customer::Customer;
use business::gateway::customer_gateway::CustomerStore;
use business::use_cases::customer_use_case::CustomerUseCase;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryCustomerStore {
    customers: Mutex<Vec<Customer>>,
}

#[async_trait]
impl CustomerStore for MemoryCustomerStore {
    async fn save(&self, mut customer: Customer) -> Result<Customer, BusinessError> {
        let mut customers = self
            .customers
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        match customer.id {
            Some(id) => {
                if let Some(existing) = customers.iter_mut().find(|row| row.id == Some(id)) {
                    *existing = customer.clone();
                } else {
                    customers.push(customer.clone());
                }
            }
            None => {
                let id = customers.len() as i64 + 1;
                customer.id = Some(id);
                customer.uuid = Some(format!("customer-{id}"));
                customers.push(customer.clone());
            }
        }
        Ok(customer)
    }

    async fn get_by_id(&self, id: i64) -> Result<Option<Customer>, BusinessError> {
        let customers = self
            .customers
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(customers
            .iter()
            .find(|customer| customer.id == Some(id))
            .cloned())
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<Customer>, BusinessError> {
        let customers = self
            .customers
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(customers
            .iter()
            .find(|customer| customer.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<Customer>, u64), BusinessError> {
        let customers = self
            .customers
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let filtered: Vec<_> = customers
            .iter()
            .filter(|customer| search.is_none_or(|term| customer.name.contains(term)))
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

fn new_customer(name: &str, tenant_id: Option<i64>) -> Customer {
    Customer {
        id: None,
        uuid: None,
        tenant_id,
        name: name.into(),
        status: " Active ".into(),
        notes: None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

#[tokio::test]
async fn customer_use_case_uses_its_store_port_for_create_find_and_page() {
    let use_case = CustomerUseCase::new(MemoryCustomerStore::default());

    let created = use_case
        .create(new_customer("  Northwind  ", Some(42)))
        .await
        .expect("valid customer is stored");
    let by_id = use_case
        .find_by_id(created.id.expect("store assigned id"))
        .await
        .expect("customer is found by id");
    let by_uuid = use_case
        .find_by_uuid(created.uuid.clone().expect("store assigned uuid"))
        .await
        .expect("customer is found by uuid");
    let (page, total) = use_case
        .find_page(0, 10, Some("North"))
        .await
        .expect("matching customers are listed");

    assert_eq!(created.name, "Northwind");
    assert_eq!(created.status, "Active");
    assert_eq!(by_id.id, created.id);
    assert_eq!(by_uuid.uuid, created.uuid);
    assert_eq!(total, 1);
    assert_eq!(page.len(), 1);
}

#[tokio::test]
async fn customer_update_preserves_persisted_tenant_and_uuid() {
    let use_case = CustomerUseCase::new(MemoryCustomerStore::default());
    let created = use_case
        .create(new_customer("Northwind", Some(42)))
        .await
        .expect("customer is stored");
    let id = created.id.expect("store assigned id");
    let uuid = created.uuid.clone();
    let mut requested_update = new_customer("  Northwind Transit  ", Some(99));
    requested_update.uuid = Some("replacement-uuid".into());

    let updated = use_case
        .update(id, requested_update)
        .await
        .expect("customer is updated");

    assert_eq!(updated.id, Some(id));
    assert_eq!(updated.uuid, uuid);
    assert_eq!(updated.tenant_id, Some(42));
    assert_eq!(updated.name, "Northwind Transit");
}

#[tokio::test]
async fn customer_use_case_rejects_a_blank_name_before_storage() {
    let use_case = CustomerUseCase::new(MemoryCustomerStore::default());
    let error = use_case
        .create(new_customer("  ", Some(42)))
        .await
        .expect_err("blank name is rejected");

    assert_eq!(error.message, "Customer name is required");
}

#[tokio::test]
async fn customer_use_case_rejects_blank_status_and_reports_missing_records() {
    let use_case = CustomerUseCase::new(MemoryCustomerStore::default());
    let mut missing_status = new_customer("Northwind", Some(42));
    missing_status.status = "  ".into();
    assert_eq!(
        use_case
            .create(missing_status)
            .await
            .expect_err("status is required")
            .message,
        "Customer status is required"
    );
    assert_eq!(
        use_case
            .find_by_id(404)
            .await
            .expect_err("unknown customer ID is reported")
            .message,
        "Customer not found"
    );
    assert_eq!(
        use_case
            .find_by_uuid("missing".into())
            .await
            .expect_err("unknown customer UUID is reported")
            .message,
        "Customer not found"
    );
    assert_eq!(
        use_case
            .update(404, new_customer("Northwind", Some(42)))
            .await
            .expect_err("unknown customer cannot be updated")
            .message,
        "Customer not found"
    );
}
