use sea_orm::entity::prelude::*;

/// `EPIC-SC-03-S01` (`HRMS-607`, `C-024`): an extra or charter trip
/// (`entity-inventory.md` §2 "Viagens extra"). **This is the Trip `D-24(f)`
/// made canonical** -- `EPIC-FO-04`'s minimal Trip was never built and is
/// not built as a separate table; this is the one shape.
///
/// Identity is `order_code` + `trip_date` (`D-24(d)`): the source system
/// reuses order codes on different dates, so code alone is not unique
/// (`uq_extra_trip_tenant_code_date`).
///
/// **Deliberately not carried from `entity-inventory.md`'s field list**:
/// `preparo_liberado_chave` (garage-prep release key) -- that is `C-028`'s
/// garage domain, which does not exist in hermes yet. Add it in the same
/// migration that builds `C-028`, not here.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "extra_trip")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub order_code: String,
    pub trip_date: Date,
    /// `cliente_nome` in `operacao-trm` is a free-text customer name; hermes
    /// already has a `Customer` registry (`EPIC-SC-01-S01`), so this
    /// references it by id instead of duplicating a name string -- fixed by
    /// design, the same reasoning that keeps `transport_demand.customer_id`
    /// a reference rather than a copy.
    pub customer_id: Option<i64>,
    pub start_time: Option<Time>,
    pub return_date: Option<Date>,
    pub return_time: Option<Time>,
    pub destination: Option<String>,
    pub origin_city: Option<String>,
    /// Free text: `entity-inventory.md` names "stops" as part of this
    /// entity's shape but states no structure for them (list? waypoints
    /// table?). Inventing one is a design decision this story has no
    /// grounds to make.
    pub stops: Option<String>,
    pub preferred_vehicle_type: Option<String>,
    pub driver_id: Option<i64>,
    pub second_driver_id: Option<i64>,
    pub vehicle_id: Option<i64>,
    pub freight_value_cents: Option<i64>,
    pub payment: Option<String>,
    /// Persisted as a `TripStatus` name -- unlike most free-text fields in
    /// this domain, `operacao-trm`'s own four values are stated in
    /// `entity-inventory.md` (`programada`/`conflito`/`pendente_escala`/
    /// `cancelada`), so this is a translation, not an invention (`PD-035`).
    pub status: String,
    pub origin: Option<String>,
    pub import_batch_id: Option<String>,
    pub imported_at: Option<DateTimeUtc>,
    /// Self-referential: the trip that replaced this one, when named.
    pub replaced_by_id: Option<i64>,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
