use sea_orm::entity::prelude::*;

/// EPIC-FO-01 (HRMS-920...925, D-20): the vehicle register — the first
/// tenant-owned table added since D-09 made the scoping rule mandatory.
/// `Eq` was dropped when `odometer_km: Option<f64>` (`C-023`) landed -- `f64`
/// has no total order, so it cannot implement `Eq`. `PartialEq` still holds.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "vehicle")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    /// HRMS-921/D-09. The column is NOT NULL in the migration, so `i64` would
    /// read more honestly here — but `impl_tenant_auditable_before_save!`
    /// stamps the scope through `TenantActiveModel::set_tenant_id(Option<i64>)`,
    /// which requires the `ActiveModel` field to be `Option<i64>`. Matching
    /// `user_entity` keeps the one macro usable by every tenant-owned table;
    /// the NOT NULL constraint is what actually refuses an unowned row.
    pub tenant_id: Option<i64>,
    pub plate: String,
    pub model: String,
    /// HRMS-922/D-23(b): persisted as the name of a
    /// `business::domain::enums::VehicleStatus`, not as a database enum type.
    pub status: String,
    /// HRMS-926: the tracking provider's device id. Platform-wide unique
    /// (`uq_vehicle_tracker_device`); set only by the platform administrator.
    pub tracker_device_id: Option<i64>,
    /// `C-023`/`HRMS-941`: operacao-trm parity fields (`entity-inventory.md`
    /// §1 "Veículos"). All nullable -- none of these have an owner-ruled
    /// vocabulary or a writer yet, `odometer_km` most of all: it is a cache
    /// of `C-025`'s kilometre-evolution record, which does not exist in
    /// hermes yet, so it stays `None` until that domain writes it.
    pub prefix: Option<String>,
    pub vehicle_type: Option<String>,
    pub odometer_km: Option<f64>,
    pub wheel_type: Option<String>,
    pub spare_tire_count: Option<i32>,
    pub spare_tire_type: Option<String>,
    pub spare_tire_notes: Option<String>,
    /// The garage's current tag for this vehicle (free text, e.g. a queue
    /// state) and who set it -- persisted as a `GarageTagOrigin` name, the
    /// same shape as `status`. Owned by `C-028`'s garage domain once it
    /// exists; `vehicle_tracking` (`AD-036`, `C-022`) must never write here
    /// -- `business/tests/ad036_boundary.rs` enforces that.
    pub garage_tag: Option<String>,
    pub garage_tag_origin: Option<String>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
