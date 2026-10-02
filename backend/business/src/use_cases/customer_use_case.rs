use crate::domain::business_error::BusinessError;
use crate::domain::customer::Customer;
use crate::gateway::customer_gateway::CustomerStore;

pub struct CustomerUseCase<G: CustomerStore> {
    gateway: G,
}

impl<G: CustomerStore> CustomerUseCase<G> {
    pub fn new(gateway: G) -> Self {
        Self { gateway }
    }

    pub async fn create(&self, customer: Customer) -> Result<Customer, BusinessError> {
        log::info!(
            "[CustomerUseCase::create] Executing for name: {}",
            customer.name
        );
        let customer = Self::validated(customer)?;
        self.gateway.save(customer).await
    }

    pub async fn update(&self, id: i64, customer: Customer) -> Result<Customer, BusinessError> {
        log::info!("[CustomerUseCase::update] Executing for customer id {}", id);
        let existing = self.find_by_id(id).await?;
        let customer = Self::validated(customer)?;

        let updated = Customer {
            id: Some(id),
            uuid: existing.uuid,
            // A customer never changes hands through an edit, same rule
            // HRMS-921 states for a vehicle.
            tenant_id: existing.tenant_id,
            name: customer.name,
            status: customer.status,
            notes: customer.notes,
            created_at: existing.created_at,
            created_by: existing.created_by,
            updated_at: None,
            updated_by: customer.updated_by,
        };

        self.gateway.save(updated).await
    }

    /// A customer without a name or a status is not a usable record.
    fn validated(customer: Customer) -> Result<Customer, BusinessError> {
        let name = customer.name.trim().to_string();
        if name.is_empty() {
            let msg = "Customer name is required".to_string();
            log::error!("[CustomerUseCase::validated] {}", msg);
            return Err(BusinessError::new(msg));
        }
        let status = customer.status.trim().to_string();
        if status.is_empty() {
            let msg = "Customer status is required".to_string();
            log::error!("[CustomerUseCase::validated] {}", msg);
            return Err(BusinessError::new(msg));
        }
        Ok(Customer {
            name,
            status,
            ..customer
        })
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Customer, BusinessError> {
        self.gateway
            .get_by_id(id)
            .await?
            .ok_or_else(|| BusinessError::new("Customer not found".to_string()))
    }

    /// `HRMS-204`/`AD-010`: the UUID is a customer's public identifier, same
    /// as every other tenant-owned resource. Owned by another tenant is
    /// simply absent (`tenant_select`), which lets the endpoint answer 404
    /// rather than 403 (`PD-034`).
    pub async fn find_by_uuid(&self, uuid: String) -> Result<Customer, BusinessError> {
        self.gateway
            .get_by_uuid(uuid)
            .await?
            .ok_or_else(|| BusinessError::new("Customer not found".to_string()))
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
        search: Option<&str>,
    ) -> Result<(Vec<Customer>, u64), BusinessError> {
        self.gateway.list_page(page, page_size, search).await
    }
}
