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
mod m20260926_000006_create_part_table;
mod m20260926_000006_1_widen_part_id_for_foreign_keys;
mod m20260926_000006z_widen_part_id_before_stock_ledger;
mod m20260926_000007_create_stock_movement_table;
mod m20260927_000001_create_purchase_order_table;
mod m20260927_000002_link_purchase_order_and_work_order_item;
mod m20260929_000001_create_vehicle_expense_table;
mod m20260929_000002_create_work_order_posting_table;
mod m20260929_000003_create_fuel_entry_table;
mod m20260929_000004_create_internal_tank_table;
mod m20260929_000005_add_fuel_entry_provider_transaction_id;
mod m20260929_000006_create_preventive_plan_table;
mod m20260929_000007_link_work_order_and_preventive_plan;
mod m20260929_000008_link_work_order_item_and_preventive_plan;
mod m20260929_000009_create_preventive_plan_extension;
mod m20260929_000010_add_preventive_plan_last_work_order;
mod m20260930_000001_create_preventive_plan_alert;
mod m20260930_000002_create_technical_inspection;
mod m20260930_000003_create_inspection_model;
mod m20260930_000004_add_fuel_entry_receipt_fields;
mod m20260930_000005_add_fuel_entry_provider_confirmed_at;
mod m20260930_000006_add_fuel_entry_soft_delete_and_unification;
mod m20260930_000007_add_vehicle_tank_capacity;
mod m20260930_000008_create_fuel_gauge_setting;
mod m20260930_000009_create_tenant_rule_setting;
mod m20260930_000010_add_reference_consumption;
mod m20260930_000011_create_garage_service_model;
mod m20260930_000012_create_garage_attendance_tables;
mod m20260930_000013_create_vehicle_presence_event;
mod m20260930_000014_add_garage_validity_settings;
mod m20260930_000015_create_garage_call_and_alert_settings;
mod m20260930_000016_add_transport_demand_kind;
mod m20260930_000017_add_garage_monitor_settings;
mod m20260930_000018_add_garage_monitor_card_limit;
mod m20260930_000019_add_garage_monitor_matrix_size;
mod m20260930_000020_add_garage_fuelling_freshness;
mod m20260930_000021_add_garage_utc_offset;
mod m20261002_000001_add_garage_service_applicability;

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
            Box::new(m20260926_000006_create_part_table::Migration),
            Box::new(m20260926_000006_1_widen_part_id_for_foreign_keys::Migration),
            Box::new(m20260926_000006z_widen_part_id_before_stock_ledger::Migration),
            Box::new(m20260926_000007_create_stock_movement_table::Migration),
            Box::new(m20260927_000001_create_purchase_order_table::Migration),
            Box::new(m20260927_000002_link_purchase_order_and_work_order_item::Migration),
            Box::new(m20260929_000001_create_vehicle_expense_table::Migration),
            Box::new(m20260929_000002_create_work_order_posting_table::Migration),
            Box::new(m20260929_000003_create_fuel_entry_table::Migration),
            Box::new(m20260929_000004_create_internal_tank_table::Migration),
            Box::new(m20260929_000005_add_fuel_entry_provider_transaction_id::Migration),
            Box::new(m20260929_000006_create_preventive_plan_table::Migration),
            Box::new(m20260929_000007_link_work_order_and_preventive_plan::Migration),
            Box::new(m20260929_000008_link_work_order_item_and_preventive_plan::Migration),
            Box::new(m20260929_000009_create_preventive_plan_extension::Migration),
            Box::new(m20260929_000010_add_preventive_plan_last_work_order::Migration),
            Box::new(m20260930_000001_create_preventive_plan_alert::Migration),
            Box::new(m20260930_000002_create_technical_inspection::Migration),
            Box::new(m20260930_000003_create_inspection_model::Migration),
            Box::new(m20260930_000004_add_fuel_entry_receipt_fields::Migration),
            Box::new(m20260930_000005_add_fuel_entry_provider_confirmed_at::Migration),
            Box::new(m20260930_000006_add_fuel_entry_soft_delete_and_unification::Migration),
            Box::new(m20260930_000007_add_vehicle_tank_capacity::Migration),
            Box::new(m20260930_000008_create_fuel_gauge_setting::Migration),
            Box::new(m20260930_000009_create_tenant_rule_setting::Migration),
            Box::new(m20260930_000010_add_reference_consumption::Migration),
            Box::new(m20260930_000011_create_garage_service_model::Migration),
            Box::new(m20260930_000012_create_garage_attendance_tables::Migration),
            Box::new(m20260930_000013_create_vehicle_presence_event::Migration),
            Box::new(m20260930_000014_add_garage_validity_settings::Migration),
            Box::new(m20260930_000015_create_garage_call_and_alert_settings::Migration),
            Box::new(m20260930_000016_add_transport_demand_kind::Migration),
            Box::new(m20260930_000017_add_garage_monitor_settings::Migration),
            Box::new(m20260930_000018_add_garage_monitor_card_limit::Migration),
            Box::new(m20260930_000019_add_garage_monitor_matrix_size::Migration),
            Box::new(m20260930_000020_add_garage_fuelling_freshness::Migration),
            Box::new(m20260930_000021_add_garage_utc_offset::Migration),
            Box::new(m20261002_000001_add_garage_service_applicability::Migration),
        ]
    }
}
