use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::vehicle_expense::{VehicleExpense, VehicleExpenseEntityMapper};
use crate::gateway::vehicle_expense_gateway::VehicleExpenseGateway;
use crate::gateway::vehicle_gateway::VehicleGateway;
use sea_orm::DbErr;

pub const CATEGORY_REQUIRED: &str = "A vehicle expense needs a category";
pub const COMPETENCE_PERIOD_REQUIRED: &str = "A vehicle expense needs a competence period";
pub const VALUE_MUST_BE_POSITIVE: &str = "A vehicle expense value must be positive";
pub const VEHICLE_NOT_FOUND: &str = "Vehicle not found";
/// `TRM-661`: "one invoice number produces at most one live expense per
/// vehicle and category."
pub const DUPLICATE_EXPENSE_INVOICE: &str =
    "This vehicle already has an expense in this category for that invoice number";
pub const VEHICLE_EXPENSE_NOT_FOUND: &str = "Vehicle expense not found";

pub struct VehicleExpenseUseCase {
    gateway: VehicleExpenseGateway,
    vehicles: VehicleGateway,
}

impl VehicleExpenseUseCase {
    pub fn new(gateway: VehicleExpenseGateway, vehicles: VehicleGateway) -> Self {
        Self { gateway, vehicles }
    }

    pub async fn create(&self, expense: VehicleExpense) -> Result<VehicleExpense, BusinessError> {
        let expense = self.validated(expense).await?;
        let entity = self
            .gateway
            .persist(expense)
            .await
            .map_err(database_error)?;
        Ok(VehicleExpenseEntityMapper::from_active_model(entity))
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

        let vehicle = self
            .vehicles
            .find_by_id(expense.vehicle_id)
            .await
            .map_err(database_error)?;
        let belongs = vehicle.is_some_and(|v| v.tenant_id == expense.tenant_id);
        if !belongs {
            return Err(BusinessError::new(VEHICLE_NOT_FOUND.to_string()));
        }

        if let Some(invoice_number) = expense
            .invoice_number
            .as_deref()
            .filter(|s| !s.trim().is_empty())
        {
            let existing = self
                .gateway
                .find_by_vehicle_category_invoice(expense.vehicle_id, &category, invoice_number)
                .await
                .map_err(database_error)?;
            if !existing.is_empty() {
                return Err(BusinessError::new(DUPLICATE_EXPENSE_INVOICE.to_string()));
            }
        }

        Ok(VehicleExpense {
            category,
            competence_period,
            ..expense
        })
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<VehicleExpense, BusinessError> {
        let entity = self
            .gateway
            .find_by_uuid(uuid)
            .await
            .map_err(database_error)?;
        match entity {
            Some(model) => Ok(VehicleExpenseEntityMapper::from_model(model)),
            None => Err(BusinessError::new(VEHICLE_EXPENSE_NOT_FOUND.to_string())),
        }
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<VehicleExpense>, u64), BusinessError> {
        let (rows, total) = self
            .gateway
            .find_page(page, page_size)
            .await
            .map_err(database_error)?;
        Ok((VehicleExpenseEntityMapper::from_models(rows), total))
    }
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[VehicleExpenseUseCase] {}", msg);
    BusinessError::new(msg)
}
