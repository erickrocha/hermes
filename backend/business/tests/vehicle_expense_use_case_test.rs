use business::domain::business_error::BusinessError;
use business::domain::enums::{ExpenseOrigin, GarageTagOrigin, VehicleStatus};
use business::domain::vehicle::Vehicle;
use business::domain::vehicle_expense::VehicleExpense;
use business::gateway::vehicle_expense_gateway::{VehicleExpenseStore, VehicleExpenseStoreError};
use business::gateway::vehicle_gateway::VehicleLookupStore;
use business::use_cases::vehicle_expense_use_case::{
    CATEGORY_REQUIRED, DUPLICATE_EXPENSE_INVOICE, VEHICLE_NOT_FOUND, VehicleExpenseUseCase,
};
use chrono::NaiveDate;
use sea_orm::prelude::async_trait::async_trait;
use std::sync::Mutex;

#[derive(Default)]
struct MemoryVehicleExpenseStore {
    expenses: Mutex<Vec<VehicleExpense>>,
}

#[async_trait]
impl VehicleExpenseStore for MemoryVehicleExpenseStore {
    async fn save(
        &self,
        mut expense: VehicleExpense,
    ) -> Result<VehicleExpense, VehicleExpenseStoreError> {
        let mut expenses = self.expenses.lock().map_err(|_| {
            VehicleExpenseStoreError::Storage(BusinessError::new("memory store poisoned".into()))
        })?;
        if expense.invoice_number.as_deref().is_some_and(|invoice| {
            expenses.iter().any(|existing| {
                existing.vehicle_id == expense.vehicle_id
                    && existing.category == expense.category
                    && existing.invoice_number.as_deref() == Some(invoice)
            })
        }) {
            return Err(VehicleExpenseStoreError::DuplicateInvoice);
        }
        let id = expense.id.unwrap_or(expenses.len() as i64 + 1);
        expense.id = Some(id);
        expense.uuid = Some(format!("vehicle-expense-{id}"));
        expenses.push(expense.clone());
        Ok(expense)
    }

    async fn invoice_exists(
        &self,
        vehicle_id: i64,
        category: &str,
        invoice_number: &str,
    ) -> Result<bool, BusinessError> {
        let expenses = self
            .expenses
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(expenses.iter().any(|expense| {
            expense.vehicle_id == vehicle_id
                && expense.category == category
                && expense.invoice_number.as_deref() == Some(invoice_number)
        }))
    }

    async fn get_by_uuid(&self, uuid: String) -> Result<Option<VehicleExpense>, BusinessError> {
        let expenses = self
            .expenses
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        Ok(expenses
            .iter()
            .find(|expense| expense.uuid.as_deref() == Some(uuid.as_str()))
            .cloned())
    }

    async fn list_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<VehicleExpense>, u64), BusinessError> {
        let expenses = self
            .expenses
            .lock()
            .map_err(|_| BusinessError::new("memory store poisoned".into()))?;
        let total = expenses.len() as u64;
        let start = (page * page_size) as usize;
        Ok((
            expenses
                .iter()
                .skip(start)
                .take(page_size as usize)
                .cloned()
                .collect(),
            total,
        ))
    }
}

struct MemoryVehicleLookup {
    vehicle: Option<Vehicle>,
}

#[async_trait]
impl VehicleLookupStore for MemoryVehicleLookup {
    async fn get_by_id(&self, id: i64) -> Result<Option<Vehicle>, BusinessError> {
        Ok(self
            .vehicle
            .as_ref()
            .filter(|vehicle| vehicle.id == Some(id))
            .cloned())
    }
}

fn vehicle(tenant_id: Option<i64>) -> Vehicle {
    Vehicle {
        id: Some(10),
        uuid: Some("vehicle-10".into()),
        tenant_id,
        plate: "ABC1D23".into(),
        model: "Truck".into(),
        status: VehicleStatus::Active,
        tracker_device_id: None,
        prefix: None,
        vehicle_type: None,
        odometer_km: None,
        wheel_type: None,
        spare_tire_count: None,
        spare_tire_type: None,
        spare_tire_notes: None,
        tank_capacity_liters: None,
        reference_km_per_liter: None,
        garage_tag: None,
        garage_tag_origin: Option::<GarageTagOrigin>::None,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn expense() -> VehicleExpense {
    VehicleExpense {
        id: None,
        uuid: None,
        tenant_id: Some(42),
        vehicle_id: 10,
        category: "  Tolls  ".into(),
        competence_period: "  2026-09  ".into(),
        issue_date: NaiveDate::from_ymd_opt(2026, 9, 29).expect("valid test date"),
        supplier: None,
        invoice_number: Some("INV-1".into()),
        description: None,
        value_cents: 5_000,
        origin: ExpenseOrigin::Manual,
        created_at: None,
        created_by: None,
        updated_at: None,
        updated_by: None,
    }
}

fn use_case(
    vehicle: Option<Vehicle>,
) -> VehicleExpenseUseCase<MemoryVehicleExpenseStore, MemoryVehicleLookup> {
    VehicleExpenseUseCase::new(
        MemoryVehicleExpenseStore::default(),
        MemoryVehicleLookup { vehicle },
    )
}

#[tokio::test]
async fn vehicle_expense_use_case_uses_store_ports_for_create_find_and_page() {
    let use_case = use_case(Some(vehicle(Some(42))));

    let created = use_case
        .create(expense())
        .await
        .expect("valid expense is stored");
    let found = use_case
        .find_by_uuid(created.uuid.clone().expect("store assigned uuid"))
        .await
        .expect("expense is found");
    let (page, total) = use_case.find_page(0, 10).await.expect("page is listed");

    assert_eq!(created.category, "Tolls");
    assert_eq!(created.competence_period, "2026-09");
    assert_eq!(found.id, created.id);
    assert_eq!(total, 1);
    assert_eq!(page.len(), 1);
}

#[tokio::test]
async fn vehicle_expense_rejects_another_tenants_vehicle() {
    let use_case = use_case(Some(vehicle(Some(99))));

    let error = use_case
        .create(expense())
        .await
        .expect_err("vehicle from another tenant is hidden");

    assert_eq!(error.message, VEHICLE_NOT_FOUND);
}

#[tokio::test]
async fn vehicle_expense_maps_duplicate_invoice_to_the_domain_error() {
    let use_case = use_case(Some(vehicle(Some(42))));
    use_case
        .create(expense())
        .await
        .expect("first expense is stored");

    let error = use_case
        .create(expense())
        .await
        .expect_err("duplicate invoice is refused");

    assert_eq!(error.message, DUPLICATE_EXPENSE_INVOICE);
}

#[tokio::test]
async fn vehicle_expense_rejects_a_blank_category_before_storage() {
    let use_case = use_case(Some(vehicle(Some(42))));
    let mut invalid = expense();
    invalid.category = "  ".into();

    let error = use_case
        .create(invalid)
        .await
        .expect_err("blank category is refused");

    assert_eq!(error.message, CATEGORY_REQUIRED);
}
