use crate::commons::functions::digits_only;
use crate::commons::tax_id::{self, TaxIdOutcome};
use crate::domain::business_error::BusinessError;
use crate::domain::customer::Customer;
use crate::gateway::customer_gateway::CustomerStore;

pub const NAME_REQUIRED: &str = "Customer name is required";
pub const STATUS_REQUIRED: &str = "Customer status is required";
/// `EPIC-SC-01-S04`: the document failed its country's check digits.
pub const INVALID_TAX_ID: &str = "Customer tax id is not a valid document";
/// `EPIC-SC-01-S04`: another customer of the tenant already holds the document.
pub const DUPLICATE_TAX_ID: &str = "This tenant already has a customer with this tax id";

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
        self.ensure_tax_id_free(&customer, None).await?;
        self.gateway.save(customer).await
    }

    pub async fn update(&self, id: i64, customer: Customer) -> Result<Customer, BusinessError> {
        log::info!("[CustomerUseCase::update] Executing for customer id {}", id);
        let existing = self.find_by_id(id).await?;
        // A customer never changes hands through an edit, same rule HRMS-921 states for a vehicle.
        let customer = Self::validated(Customer { tenant_id: existing.tenant_id, ..customer })?;
        self.ensure_tax_id_free(&customer, Some(id)).await?;

        let updated = Customer {
            id: Some(id),
            uuid: existing.uuid,
            created_at: existing.created_at,
            created_by: existing.created_by,
            updated_at: None,
            ..customer
        };

        self.gateway.save(updated).await
    }

    /// A customer without a name or a status is not a usable record. `EPIC-SC-01-S04`: the taxpayer
    /// document, when given, is validated and normalised by the country's rule (`PD-022` -- a country
    /// with no validator keeps it as typed); the postal code is digits only; blank optional fields are
    /// stored as absent.
    fn validated(customer: Customer) -> Result<Customer, BusinessError> {
        let name = customer.name.trim().to_string();
        if name.is_empty() {
            log::error!("[CustomerUseCase::validated] {}", NAME_REQUIRED);
            return Err(BusinessError::new(NAME_REQUIRED.to_string()));
        }
        let status = customer.status.trim().to_string();
        if status.is_empty() {
            log::error!("[CustomerUseCase::validated] {}", STATUS_REQUIRED);
            return Err(BusinessError::new(STATUS_REQUIRED.to_string()));
        }
        let country_code = optional(customer.country_code).map(|c| c.to_ascii_uppercase());
        let tax_id = match optional(customer.tax_id) {
            None => None,
            Some(raw) => match tax_id::validate_and_normalize(country_code.as_deref().unwrap_or("BR"), &raw) {
                TaxIdOutcome::Valid(digits) => Some(digits),
                TaxIdOutcome::Unsupported => Some(raw),
                TaxIdOutcome::Invalid => {
                    log::error!("[CustomerUseCase::validated] {}", INVALID_TAX_ID);
                    return Err(BusinessError::new(INVALID_TAX_ID.to_string()));
                }
            },
        };
        Ok(Customer {
            name,
            status,
            legal_name: optional(customer.legal_name),
            tax_id,
            address_line1: optional(customer.address_line1),
            address_line2: optional(customer.address_line2),
            locality: optional(customer.locality),
            administrative_area: optional(customer.administrative_area),
            postal_code: digits_only(customer.postal_code),
            country_code,
            notes: optional(customer.notes),
            ..customer
        })
    }

    /// `EPIC-SC-01-S04`: one document, one customer per tenant. `self_id` is the customer being edited.
    async fn ensure_tax_id_free(&self, customer: &Customer, self_id: Option<i64>) -> Result<(), BusinessError> {
        let Some(tax_id) = customer.tax_id.as_deref() else {
            return Ok(());
        };
        let taken = self
            .gateway
            .get_by_tax_id(customer.tenant_id, tax_id)
            .await?
            .is_some_and(|other| other.id != self_id);
        if taken {
            log::error!("[CustomerUseCase::ensure_tax_id_free] {}", DUPLICATE_TAX_ID);
            return Err(BusinessError::new(DUPLICATE_TAX_ID.to_string()));
        }
        Ok(())
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

fn optional(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}
