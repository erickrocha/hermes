pub use sea_orm_migration::{async_trait, MigratorTrait, MigrationTrait};

mod reference_data;

mod m20260916_000001_create_table_user;
mod m20260916_000004_create_tenant_table;
mod m20260916_000002_create_business_plan_table;
mod m20260916_000003_business_plan_tiers;
mod m20260916_000008_data_load_us_provinces_and_cities;
mod m20260916_000006_create_province_table;
mod m20260916_000007_create_city_table;
mod m20260917_000001_tenant_business_plan_fk;
mod m20260917_000002_data_load_br_provinces_and_cities;
mod m20260917_000004_tenant_country_required_and_tax_id_unique;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260916_000001_create_table_user::Migration),
            Box::new(m20260916_000002_create_business_plan_table::Migration),
            Box::new(m20260916_000003_business_plan_tiers::Migration),
            Box::new(m20260916_000004_create_tenant_table::Migration),
            Box::new(m20260916_000006_create_province_table::Migration),
            Box::new(m20260916_000007_create_city_table::Migration),
            Box::new(m20260916_000008_data_load_us_provinces_and_cities::Migration),
            Box::new(m20260917_000001_tenant_business_plan_fk::Migration),
            Box::new(m20260917_000002_data_load_br_provinces_and_cities::Migration),
            Box::new(m20260917_000004_tenant_country_required_and_tax_id_unique::Migration),
        ]
    }
}