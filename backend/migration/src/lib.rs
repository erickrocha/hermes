pub use sea_orm_migration::{async_trait, MigratorTrait, MigrationTrait};

mod reference_data;

mod m20260916_000001_create_table_user;
mod m20260916_000003_business_plan_tiers;
mod m20260916_000004_create_tenant_table;
mod m20260916_000005_create_tenant_plan_table;
mod m20260916_000002_create_business_plan_table;
mod m20260916_000008_data_load_us_provinces_and_cities;
mod m20260916_000006_create_province_table;
mod m20260916_000007_create_city_table;
mod m20260917_000001_tenant_business_plan_fk;
mod m20260917_000002_data_load_br_provinces_and_cities;
mod m20260917_000004_tenant_country_required_and_tax_id_unique;
mod m20260918_000001_sysadmin_has_no_tenant;
mod m20260918_000002_tenant_column_names;
mod m20260918_000003_reconcile_edited_migrations;
mod m20260919_000001_widen_audit_actor_columns;
mod m20260921_000001_create_vehicle_table;
mod m20260924_000001_add_vehicle_tracker_device;
mod m20260924_000002_create_vehicle_assignment_table;
mod m20260925_000001_add_vehicle_parity_fields;
mod m20260925_000002_create_customer_table;
mod m20260925_000003_create_customer_day_off_table;
mod m20260925_000004_create_holiday_table;
mod m20260925_000005_create_transport_demand_table;
mod m20260925_000006_create_transport_demand_allocation_table;
mod m20260925_000007_create_daily_schedule_table;
mod m20260925_000008_create_schedule_exception_table;
mod m20260925_000009_create_extra_trip_table;
mod m20260925_000010_create_km_evolution_table;
mod m20260925_000011_create_checklist_template_table;
mod m20260925_000012_create_checklist_run_table;
mod m20260925_000013_create_checklist_answer_table;
mod m20260926_000001_create_work_order_table;
mod m20260926_000002_create_work_order_item_table;
mod m20260926_000003_link_work_order_and_checklist_answer;
mod m20260926_000004_create_maintenance_plan_table;
mod m20260926_000005_create_service_catalogue_tables;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260916_000001_create_table_user::Migration),
            Box::new(m20260916_000002_create_business_plan_table::Migration),
            Box::new(m20260916_000003_business_plan_tiers::Migration),
            Box::new(m20260916_000004_create_tenant_table::Migration),
            Box::new(m20260916_000005_create_tenant_plan_table::Migration),
            Box::new(m20260916_000006_create_province_table::Migration),
            Box::new(m20260916_000007_create_city_table::Migration),
            Box::new(m20260916_000008_data_load_us_provinces_and_cities::Migration),
            Box::new(m20260917_000001_tenant_business_plan_fk::Migration),
            Box::new(m20260917_000002_data_load_br_provinces_and_cities::Migration),
            Box::new(m20260917_000004_tenant_country_required_and_tax_id_unique::Migration),
            Box::new(m20260918_000001_sysadmin_has_no_tenant::Migration),
            Box::new(m20260918_000002_tenant_column_names::Migration),
            Box::new(m20260918_000003_reconcile_edited_migrations::Migration),
            Box::new(m20260919_000001_widen_audit_actor_columns::Migration),
            Box::new(m20260921_000001_create_vehicle_table::Migration),
            Box::new(m20260924_000001_add_vehicle_tracker_device::Migration),
            Box::new(m20260924_000002_create_vehicle_assignment_table::Migration),
            Box::new(m20260925_000001_add_vehicle_parity_fields::Migration),
            Box::new(m20260925_000002_create_customer_table::Migration),
            Box::new(m20260925_000003_create_customer_day_off_table::Migration),
            Box::new(m20260925_000004_create_holiday_table::Migration),
            Box::new(m20260925_000005_create_transport_demand_table::Migration),
            Box::new(m20260925_000006_create_transport_demand_allocation_table::Migration),
            Box::new(m20260925_000007_create_daily_schedule_table::Migration),
            Box::new(m20260925_000008_create_schedule_exception_table::Migration),
            Box::new(m20260925_000009_create_extra_trip_table::Migration),
            Box::new(m20260925_000010_create_km_evolution_table::Migration),
            Box::new(m20260925_000011_create_checklist_template_table::Migration),
            Box::new(m20260925_000012_create_checklist_run_table::Migration),
            Box::new(m20260925_000013_create_checklist_answer_table::Migration),
            Box::new(m20260926_000001_create_work_order_table::Migration),
            Box::new(m20260926_000002_create_work_order_item_table::Migration),
            Box::new(m20260926_000003_link_work_order_and_checklist_answer::Migration),
            Box::new(m20260926_000004_create_maintenance_plan_table::Migration),
            Box::new(m20260926_000005_create_service_catalogue_tables::Migration),
        ]
    }
}