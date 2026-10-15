use crate::domain::business_error::BusinessError;
use crate::domain::vehicle_expense::VehicleExpense;
use crate::gateway::vehicle_expense_gateway::{VehicleExpenseStore, VehicleExpenseStoreError};
use crate::gateway::vehicle_gateway::VehicleLookupStore;

pub const CATEGORY_REQUIRED: &str = "A vehicle expense needs a category";
pub const COMPETENCE_PERIOD_REQUIRED: &str = "A vehicle expense needs a competence period";
pub const VALUE_MUST_BE_POSITIVE: &str = "A vehicle expense value must be positive";
pub const VEHICLE_NOT_FOUND: &str = "Vehicle not found";
/// `TRM-661`: "one invoice number produces at most one live expense per
/// vehicle and category."
pub const DUPLICATE_EXPENSE_INVOICE: &str =
    "This vehicle already has an expense in this category for that invoice number";
pub const VEHICLE_EXPENSE_NOT_FOUND: &str = "Vehicle expense not found";

pub struct VehicleExpenseUseCase<E: VehicleExpenseStore, V: VehicleLookupStore> {
    gateway: E,
    vehicles: V,
}

impl<E: VehicleExpenseStore, V: VehicleLookupStore> VehicleExpenseUseCase<E, V> {
    pub fn new(gateway: E, vehicles: V) -> Self {
        Self { gateway, vehicles }
    }

    pub async fn create(&self, expense: VehicleExpense) -> Result<VehicleExpense, BusinessError> {
        let expense = self.validated(expense).await?;
        self.gateway.save(expense).await.map_err(store_error)
    }

    async fn validated(&self, expense: VehicleExpense) -> Result<VehicleExpense, BusinessError> {
        let category = expense.category.trim().to_string();
        if category.is_empty() {
            return Err(BusinessError::new(CATEGORY_REQUIRED.to_string()));
        }
        let competence_period = expense.competence_period.trim().to_string();
        if competence_period.is_empty() {
            return Err(BusinessError::new(COMPETENCE_PERIOD_REQUIRED.to_string()));
        }
        if expense.value_cents <= 0 {
            return Err(BusinessError::new(VALUE_MUST_BE_POSITIVE.to_string()));
        }

        let vehicle = self.vehicles.get_by_id(expense.vehicle_id).await?;
        let belongs = vehicle.is_some_and(|v| v.tenant_id == expense.tenant_id);
        if !belongs {
            return Err(BusinessError::new(VEHICLE_NOT_FOUND.to_string()));
        }

        if let Some(invoice_number) = expense
            .invoice_number
            .as_deref()
            .filter(|s| !s.trim().is_empty())
            && self
                .gateway
                .invoice_exists(expense.vehicle_id, &category, invoice_number)
                .await?
        {
            return Err(BusinessError::new(DUPLICATE_EXPENSE_INVOICE.to_string()));
        }

        Ok(VehicleExpense {
            category,
            competence_period,
            ..expense
        })
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<VehicleExpense, BusinessError> {
        self.gateway
            .get_by_uuid(uuid)
            .await?
            .ok_or_else(|| BusinessError::new(VEHICLE_EXPENSE_NOT_FOUND.to_string()))
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<VehicleExpense>, u64), BusinessError> {
        self.gateway.list_page(page, page_size).await
    }
}

fn store_error(error: VehicleExpenseStoreError) -> BusinessError {
    match error {
        VehicleExpenseStoreError::DuplicateInvoice => {
            BusinessError::new(DUPLICATE_EXPENSE_INVOICE.to_string())
        }
        VehicleExpenseStoreError::Storage(error) => error,
    }
}
