use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;
use utoipa::ToSchema;

#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum Role {
    SysAdmin,
    TenantOwner,
    #[default]
    TenantUser,
    /// EPIC-IA-09 (HRMS-129, PD-026, D-22): works through the driver app;
    /// tenant-bound, created by the tenant owner.
    Driver,
    /// EPIC-IA-09 (HRMS-129, PD-026, D-22): garage and maintenance only;
    /// tenant-bound, created by the tenant owner.
    Mechanic,
}

impl Display for Role {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::SysAdmin => write!(f, "SysAdmin"),
            Role::TenantOwner => write!(f, "TenantOwner"),
            Role::TenantUser => write!(f, "TenantUser"),
            Role::Driver => write!(f, "Driver"),
            Role::Mechanic => write!(f, "Mechanic"),
        }
    }
}

impl FromStr for Role {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "SysAdmin" => Ok(Self::SysAdmin),
            "TenantOwner" => Ok(Self::TenantOwner),
            "TenantUser" => Ok(Self::TenantUser),
            "Driver" => Ok(Self::Driver),
            "Mechanic" => Ok(Self::Mechanic),
            _ => Err(format!("Invalid role: {}", value))?,
        }
    }
}

/// EPIC-FO-01-S03 (HRMS-922, D-23(b)): the stated vocabulary a vehicle's
/// status is drawn from, so two depots cannot invent two spellings of the
/// same state. The five values were ratified by the product owner on
/// 2026-09-21; adding a sixth is a decision, not a spelling.
///
/// Persisted as the variant's own name (see `vehicle_entity::Model::status`),
/// deliberately without `DeriveActiveEnum` and without a database enum type,
/// exactly like `Role`.
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum VehicleStatus {
    #[default]
    Active,
    Maintenance,
    Transit,
    Reserved,
    Inactive,
}

impl Display for VehicleStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            VehicleStatus::Active => write!(f, "Active"),
            VehicleStatus::Maintenance => write!(f, "Maintenance"),
            VehicleStatus::Transit => write!(f, "Transit"),
            VehicleStatus::Reserved => write!(f, "Reserved"),
            VehicleStatus::Inactive => write!(f, "Inactive"),
        }
    }
}

impl FromStr for VehicleStatus {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Active" => Ok(Self::Active),
            "Maintenance" => Ok(Self::Maintenance),
            "Transit" => Ok(Self::Transit),
            "Reserved" => Ok(Self::Reserved),
            "Inactive" => Ok(Self::Inactive),
            _ => Err(format!("Invalid vehicle status: {}", value)),
        }
    }
}

/// `EPIC-FO-06-S01` (`HRMS-941`, `C-023`): who set a vehicle's garage tag --
/// a person, the tracker-driven automation, or a fully automatic rule.
/// Translated verbatim from `operacao-trm`'s own three values (`manual`,
/// `rastreador`, `automatico`, `entity-inventory.md` §1), per `PD-035`: this
/// is a stated vocabulary already ratified by production use, not a fresh
/// decision `C-023` has to raise.
///
/// Unlike `VehicleStatus`, absence is not defaulted to a value -- a vehicle
/// with no garage tag yet has no origin either, so this is `Option` at every
/// layer down to the column, never `#[default]`.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize, ToSchema)]
pub enum GarageTagOrigin {
    Manual,
    Tracker,
    Automatic,
}

impl Display for GarageTagOrigin {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            GarageTagOrigin::Manual => write!(f, "Manual"),
            GarageTagOrigin::Tracker => write!(f, "Tracker"),
            GarageTagOrigin::Automatic => write!(f, "Automatic"),
        }
    }
}

impl FromStr for GarageTagOrigin {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Manual" => Ok(Self::Manual),
            "Tracker" => Ok(Self::Tracker),
            "Automatic" => Ok(Self::Automatic),
            _ => Err(format!("Invalid garage tag origin: {}", value)),
        }
    }
}

/// `EPIC-SC-02-S04` (`HRMS-606`, `C-024`): the three kinds of day exception
/// `EPIC-SC-02-S04`'s own accepted user story names verbatim ("cancellation,
/// de-allocation or substitution") -- unlike most free-text fields in the
/// scheduling domain, this vocabulary is not invented here; it is the story
/// text itself.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize, ToSchema)]
pub enum ScheduleExceptionType {
    Cancellation,
    Deallocation,
    Substitution,
}

impl Display for ScheduleExceptionType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ScheduleExceptionType::Cancellation => write!(f, "Cancellation"),
            ScheduleExceptionType::Deallocation => write!(f, "Deallocation"),
            ScheduleExceptionType::Substitution => write!(f, "Substitution"),
        }
    }
}

impl FromStr for ScheduleExceptionType {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Cancellation" => Ok(Self::Cancellation),
            "Deallocation" => Ok(Self::Deallocation),
            "Substitution" => Ok(Self::Substitution),
            _ => Err(format!("Invalid schedule exception type: {}", value)),
        }
    }
}

/// `EPIC-SC-03-S01` (`HRMS-607`, `C-024`, `D-24(f)`): `operacao-trm`'s own
/// four trip states (`entity-inventory.md` §2 "Viagens extra": `programada`
/// / `conflito` / `pendente_escala` / `cancelada`), translated verbatim --
/// `PD-035`, the same shape `GarageTagOrigin` used for `tag_garagem_origem`.
/// This is a stated vocabulary already ratified by production use, not a
/// fresh decision this story has to raise.
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum TripStatus {
    #[default]
    Scheduled,
    Conflict,
    PendingSchedule,
    Cancelled,
}

impl Display for TripStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            TripStatus::Scheduled => write!(f, "Scheduled"),
            TripStatus::Conflict => write!(f, "Conflict"),
            TripStatus::PendingSchedule => write!(f, "PendingSchedule"),
            TripStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

impl FromStr for TripStatus {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Scheduled" => Ok(Self::Scheduled),
            "Conflict" => Ok(Self::Conflict),
            "PendingSchedule" => Ok(Self::PendingSchedule),
            "Cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("Invalid trip status: {}", value)),
        }
    }
}

/// `EPIC-CK-01-S01` (`HRMS-650`, `C-025`, `AD-041`): `TRM-151`'s own
/// vocabulary, stated in English already -- *"restrict the origin of a
/// kilometre entry to the set: initial registration, manual, work order,
/// driver checklist, garage, technical inspection, adjustment."* Not
/// invented here. `"abastecimento"` (fuel) is deliberately not an eighth
/// value: `D-29` (`codebase-survey-combustivel-estoque.md`) names it a
/// bypass defect, not a value the vocabulary ever sanctioned.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize, ToSchema)]
pub enum KmOrigin {
    InitialRegistration,
    Manual,
    WorkOrder,
    DriverChecklist,
    Garage,
    TechnicalInspection,
    Adjustment,
}

impl Display for KmOrigin {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            KmOrigin::InitialRegistration => write!(f, "InitialRegistration"),
            KmOrigin::Manual => write!(f, "Manual"),
            KmOrigin::WorkOrder => write!(f, "WorkOrder"),
            KmOrigin::DriverChecklist => write!(f, "DriverChecklist"),
            KmOrigin::Garage => write!(f, "Garage"),
            KmOrigin::TechnicalInspection => write!(f, "TechnicalInspection"),
            KmOrigin::Adjustment => write!(f, "Adjustment"),
        }
    }
}

impl FromStr for KmOrigin {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "InitialRegistration" => Ok(Self::InitialRegistration),
            "Manual" => Ok(Self::Manual),
            "WorkOrder" => Ok(Self::WorkOrder),
            "DriverChecklist" => Ok(Self::DriverChecklist),
            "Garage" => Ok(Self::Garage),
            "TechnicalInspection" => Ok(Self::TechnicalInspection),
            "Adjustment" => Ok(Self::Adjustment),
            _ => Err(format!("Invalid km origin: {}", value)),
        }
    }
}

/// `EPIC-CK-02-S01` (`HRMS-651`, `C-025`): `entity-inventory.md` §5
/// "Checklists do motorista" own wording -- *"departure / return /
/// standalone checklist"* -- is the stated vocabulary for both a checklist
/// template's `tipo_checklist` and, once `EPIC-CK-03` exists, a checklist
/// run's own type. Not invented here.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize, ToSchema)]
pub enum ChecklistType {
    Departure,
    Return,
    Standalone,
}

impl Display for ChecklistType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ChecklistType::Departure => write!(f, "Departure"),
            ChecklistType::Return => write!(f, "Return"),
            ChecklistType::Standalone => write!(f, "Standalone"),
        }
    }
}

impl FromStr for ChecklistType {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Departure" => Ok(Self::Departure),
            "Return" => Ok(Self::Return),
            "Standalone" => Ok(Self::Standalone),
            _ => Err(format!("Invalid checklist type: {}", value)),
        }
    }
}

/// `EPIC-CK-04-S01` (`HRMS-654`, `C-025`): `TRM-115`/`TRM-116`'s own
/// repeated term for a failing checklist item -- "non-conforming" -- is
/// this two-value vocabulary's source; `Conforming` is its stated opposite.
/// No third value is documented anywhere in the project truth.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize, ToSchema)]
pub enum AnswerStatus {
    Conforming,
    NonConforming,
}

impl Display for AnswerStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AnswerStatus::Conforming => write!(f, "Conforming"),
            AnswerStatus::NonConforming => write!(f, "NonConforming"),
        }
    }
}

impl FromStr for AnswerStatus {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Conforming" => Ok(Self::Conforming),
            "NonConforming" => Ok(Self::NonConforming),
            _ => Err(format!("Invalid answer status: {}", value)),
        }
    }
}

/// `EPIC-MT-01-S01` (`HRMS-700`, `C-026`): the five states `entity-inventory.md`
/// §4 names for `ordemServicos.status` (`aberta / parcialmente resolvida /
/// aguardando peça / concluida / cancelada`). The literal closed set is
/// entity-inventory's own inference from code -- `product-requirements.md`
/// never quotes the five strings together -- but each concept is
/// individually confirmed by requirement text: `TRM-206` (awaiting parts),
/// `TRM-207` (partially resolved), `TRM-208`/`TRM-214` (concluded is an
/// administrative act), `TRM-205` (cancellation). Treated as authoritative
/// absent contrary evidence, the same evidentiary standard already applied
/// to `ChecklistType`. A new work order always opens `Open` (`TRM-202`).
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum WorkOrderStatus {
    #[default]
    Open,
    PartiallyResolved,
    AwaitingParts,
    Concluded,
    Cancelled,
}

impl Display for WorkOrderStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkOrderStatus::Open => write!(f, "Open"),
            WorkOrderStatus::PartiallyResolved => write!(f, "PartiallyResolved"),
            WorkOrderStatus::AwaitingParts => write!(f, "AwaitingParts"),
            WorkOrderStatus::Concluded => write!(f, "Concluded"),
            WorkOrderStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

impl FromStr for WorkOrderStatus {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Open" => Ok(Self::Open),
            "PartiallyResolved" => Ok(Self::PartiallyResolved),
            "AwaitingParts" => Ok(Self::AwaitingParts),
            "Concluded" => Ok(Self::Concluded),
            "Cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("Invalid work order status: {}", value)),
        }
    }
}

/// `EPIC-MT-01-S02` (`HRMS-701`, `C-026`): the four states
/// `entity-inventory.md` §4 names for `ordemServicoItens.status` (`pendente
/// / resolvido / cancelado / aguardando peça`). A new item always starts
/// `Pending` (`TRM-203`).
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum WorkOrderItemStatus {
    #[default]
    Pending,
    Resolved,
    Cancelled,
    AwaitingParts,
}

impl Display for WorkOrderItemStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkOrderItemStatus::Pending => write!(f, "Pending"),
            WorkOrderItemStatus::Resolved => write!(f, "Resolved"),
            WorkOrderItemStatus::Cancelled => write!(f, "Cancelled"),
            WorkOrderItemStatus::AwaitingParts => write!(f, "AwaitingParts"),
        }
    }
}

impl FromStr for WorkOrderItemStatus {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Pending" => Ok(Self::Pending),
            "Resolved" => Ok(Self::Resolved),
            "Cancelled" => Ok(Self::Cancelled),
            "AwaitingParts" => Ok(Self::AwaitingParts),
            _ => Err(format!("Invalid work order item status: {}", value)),
        }
    }
}

/// `EPIC-CK-03-S02` (`HRMS-653`, `C-025`/`C-026`): a work order's origin,
/// grown to a real vocabulary now that a second producer exists --
/// `maintenance-work-orders_implementation_plan.md` deliberately left this a
/// server-set string in `EPIC-MT-01-S01` because only one producer
/// (`Manual`) existed then, promising to grow it "the same day its producer
/// is built." `Checklist` is that day: a driver checklist's flagged,
/// non-conforming answers open the work order (`TRM-115`/`TRM-116`). Never
/// client-supplied -- only the server-side code path that creates a work
/// order decides it.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize, ToSchema)]
pub enum WorkOrderOrigin {
    Manual,
    Checklist,
    Preventive,
    Inspection,
}

impl Display for WorkOrderOrigin {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkOrderOrigin::Manual => write!(f, "Manual"),
            WorkOrderOrigin::Checklist => write!(f, "Checklist"),
            WorkOrderOrigin::Preventive => write!(f, "Preventive"),
            WorkOrderOrigin::Inspection => write!(f, "Inspection"),
        }
    }
}

impl FromStr for WorkOrderOrigin {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Manual" => Ok(Self::Manual),
            "Checklist" => Ok(Self::Checklist),
            "Preventive" => Ok(Self::Preventive),
            "Inspection" => Ok(Self::Inspection),
            _ => Err(format!("Invalid work order origin: {}", value)),
        }
    }
}

/// `EPIC-MT-03-S01` (`HRMS-703`, `C-026`): a maintenance plan's lifecycle
/// state. `Scheduled` is the default, active state; `Concluded`/`Cancelled`/
/// `NotExecuted` are the three states `TRM-237` names as inactive when
/// deciding which work orders are already scheduled. `NotExecuted` itself is
/// set only by `EPIC-MT-04`'s non-execution automation (not built yet) --
/// the value exists here because `TRM-237` already names it as one of a
/// plan's possible states, not because this story sets it.
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum MaintenancePlanStatus {
    #[default]
    Scheduled,
    Concluded,
    Cancelled,
    NotExecuted,
}

impl Display for MaintenancePlanStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MaintenancePlanStatus::Scheduled => write!(f, "Scheduled"),
            MaintenancePlanStatus::Concluded => write!(f, "Concluded"),
            MaintenancePlanStatus::Cancelled => write!(f, "Cancelled"),
            MaintenancePlanStatus::NotExecuted => write!(f, "NotExecuted"),
        }
    }
}

impl FromStr for MaintenancePlanStatus {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Scheduled" => Ok(Self::Scheduled),
            "Concluded" => Ok(Self::Concluded),
            "Cancelled" => Ok(Self::Cancelled),
            "NotExecuted" => Ok(Self::NotExecuted),
            _ => Err(format!("Invalid maintenance plan status: {}", value)),
        }
    }
}

/// `EPIC-SP-03-S01` (`HRMS-802`, `C-030`): `TRM-641` -- a purchase order is
/// "active" (`is_active`) while it is neither `Purchased` nor `Cancelled`.
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum PurchaseOrderStatus {
    #[default]
    Requested,
    Ordered,
    Purchased,
    Cancelled,
}

impl PurchaseOrderStatus {
    pub fn is_active(&self) -> bool {
        !matches!(
            self,
            PurchaseOrderStatus::Purchased | PurchaseOrderStatus::Cancelled
        )
    }
}

impl Display for PurchaseOrderStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PurchaseOrderStatus::Requested => write!(f, "Requested"),
            PurchaseOrderStatus::Ordered => write!(f, "Ordered"),
            PurchaseOrderStatus::Purchased => write!(f, "Purchased"),
            PurchaseOrderStatus::Cancelled => write!(f, "Cancelled"),
        }
    }
}

impl FromStr for PurchaseOrderStatus {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Requested" => Ok(Self::Requested),
            "Ordered" => Ok(Self::Ordered),
            "Purchased" => Ok(Self::Purchased),
            "Cancelled" => Ok(Self::Cancelled),
            _ => Err(format!("Invalid purchase order status: {}", value)),
        }
    }
}

/// `EPIC-SP-02-S01` (`HRMS-801`, `C-030`): `TRM-602`/`603` -- the three
/// movement kinds a part's stock ledger holds. No producer for `Issue` exists
/// in this epic; `EPIC-MT-02` (blocked on this Change) is its own producer,
/// writing directly through `StockMovementGateway` with this same vocabulary
/// rather than duplicating it. `Adjustment` degrades an unrecognised stored
/// value, the same "unknown persisted string" case every enum in this module
/// degrades rather than panics on -- it is the safest of the three since
/// `quantity`'s own sign, not the type label, carries the stock effect.
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum StockMovementType {
    Entry,
    Issue,
    #[default]
    Adjustment,
}

impl Display for StockMovementType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            StockMovementType::Entry => write!(f, "Entry"),
            StockMovementType::Issue => write!(f, "Issue"),
            StockMovementType::Adjustment => write!(f, "Adjustment"),
        }
    }
}

impl FromStr for StockMovementType {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Entry" => Ok(Self::Entry),
            "Issue" => Ok(Self::Issue),
            "Adjustment" => Ok(Self::Adjustment),
            _ => Err(format!("Invalid stock movement type: {}", value)),
        }
    }
}

/// `EPIC-SP-02-S01` (`HRMS-801`): `TRM-608`/`609` -- which of a part's three
/// cost tiers supplied a posting's value, or `None` when none did (an entry
/// states its own value directly rather than looking one up, so it is
/// recorded as `Informed`, not one of the three lookup tiers).
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum CostSource {
    Informed,
    MovingAverageCost,
    LastPurchasePrice,
    RegisteredUnitValue,
    #[default]
    None,
}

impl Display for CostSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            CostSource::Informed => write!(f, "Informed"),
            CostSource::MovingAverageCost => write!(f, "MovingAverageCost"),
            CostSource::LastPurchasePrice => write!(f, "LastPurchasePrice"),
            CostSource::RegisteredUnitValue => write!(f, "RegisteredUnitValue"),
            CostSource::None => write!(f, "None"),
        }
    }
}

impl FromStr for CostSource {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Informed" => Ok(Self::Informed),
            "MovingAverageCost" => Ok(Self::MovingAverageCost),
            "LastPurchasePrice" => Ok(Self::LastPurchasePrice),
            "RegisteredUnitValue" => Ok(Self::RegisteredUnitValue),
            "None" => Ok(Self::None),
            _ => Err(format!("Invalid cost source: {}", value)),
        }
    }
}

/// `EPIC-SP-04-S01` (`HRMS-803`, `C-030`): `TRM-660` lists "origin" as one of
/// a vehicle expense's own fields. No producer for `Import` exists in this
/// epic -- `EPIC-SP-06`'s toll-invoice OCR import is its own producer,
/// writing through this same vocabulary rather than a parallel one, the same
/// "grow the vocabulary the day its second producer exists" discipline
/// `StockMovementType::Issue` already established.
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum ExpenseOrigin {
    #[default]
    Manual,
    Import,
}

impl Display for ExpenseOrigin {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ExpenseOrigin::Manual => write!(f, "Manual"),
            ExpenseOrigin::Import => write!(f, "Import"),
        }
    }
}

impl FromStr for ExpenseOrigin {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Manual" => Ok(Self::Manual),
            "Import" => Ok(Self::Import),
            _ => Err(format!("Invalid expense origin: {}", value)),
        }
    }
}

/// `EPIC-FU-01-S01` (`HRMS-942`, `C-029`): `TRM-514` lists "origin" as one of
/// a fuelling's own fields; legacy's vocabulary is `cta`/`cta_sync`/
/// `motorista_foto`/`manual`. Declared in full now -- `CtaSync`/`DriverPhoto`
/// have no producer in this epic (`EPIC-FU-02`/`03` are their own, not yet
/// built) -- the same "grow the vocabulary the day its second producer
/// exists" discipline `StockMovementType::Issue` already established, cheap
/// to declare up front and costly to migrate onto later.
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum FuelEntryOrigin {
    #[default]
    Manual,
    CtaSync,
    DriverPhoto,
}

impl Display for FuelEntryOrigin {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            FuelEntryOrigin::Manual => write!(f, "Manual"),
            FuelEntryOrigin::CtaSync => write!(f, "CtaSync"),
            FuelEntryOrigin::DriverPhoto => write!(f, "DriverPhoto"),
        }
    }
}

impl FromStr for FuelEntryOrigin {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Manual" => Ok(Self::Manual),
            "CtaSync" => Ok(Self::CtaSync),
            "DriverPhoto" => Ok(Self::DriverPhoto),
            _ => Err(format!("Invalid fuel entry origin: {}", value)),
        }
    }
}

/// `EPIC-MT-07-S01` (`HRMS-706`, `C-027`): `TRM-301` -- a preventive plan is
/// controlled by kilometres, by date, or by both.
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum PreventiveControlType {
    Kilometers,
    Days,
    #[default]
    Both,
}

impl Display for PreventiveControlType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PreventiveControlType::Kilometers => write!(f, "Kilometers"),
            PreventiveControlType::Days => write!(f, "Days"),
            PreventiveControlType::Both => write!(f, "Both"),
        }
    }
}

impl FromStr for PreventiveControlType {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Kilometers" => Ok(Self::Kilometers),
            "Days" => Ok(Self::Days),
            "Both" => Ok(Self::Both),
            _ => Err(format!("Invalid preventive control type: {}", value)),
        }
    }
}

/// `EPIC-MT-07-S01` (`TRM-302`/`303`): a preventive plan's own status, always
/// derived at read time from the vehicle's current odometer and today's date
/// -- never stored, the same "computed, never a stored figure that can drift"
/// bias `TRM-602` already established for stock. Ordered worst-first so a
/// caller can take the max across several plans.
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum PreventiveStatus {
    #[default]
    Ok,
    Attention,
    Overdue,
}

impl Display for PreventiveStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PreventiveStatus::Ok => write!(f, "Ok"),
            PreventiveStatus::Attention => write!(f, "Attention"),
            PreventiveStatus::Overdue => write!(f, "Overdue"),
        }
    }
}

/// `EPIC-GA-01-S01` (`HRMS-956`, `TRM-431`): a garage service is external
/// (washing, fuelling...) or internal (cleaning, WC...), recorded explicitly --
/// never inferred from its name.
#[derive(Clone, Eq, PartialEq, Debug, Serialize, Deserialize, ToSchema)]
pub enum GarageServiceGroup {
    External,
    Internal,
}

impl Display for GarageServiceGroup {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            GarageServiceGroup::External => write!(f, "External"),
            GarageServiceGroup::Internal => write!(f, "Internal"),
        }
    }
}

impl FromStr for GarageServiceGroup {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "External" => Ok(Self::External),
            "Internal" => Ok(Self::Internal),
            _ => Err(format!("Invalid garage service group: {}", value)),
        }
    }
}

/// `EPIC-GA-02-S01` (`HRMS-958`, `TRM-411`): a triage is active until it reaches one of the three terminal states.
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum GarageAttendanceStatus {
    #[default]
    Open,
    Finished,
    ReleasedWithPendency,
    LeftForOperation,
}

impl Display for GarageAttendanceStatus {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            GarageAttendanceStatus::Open => write!(f, "Open"),
            GarageAttendanceStatus::Finished => write!(f, "Finished"),
            GarageAttendanceStatus::ReleasedWithPendency => write!(f, "ReleasedWithPendency"),
            GarageAttendanceStatus::LeftForOperation => write!(f, "LeftForOperation"),
        }
    }
}

impl FromStr for GarageAttendanceStatus {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Open" => Ok(Self::Open),
            "Finished" => Ok(Self::Finished),
            "ReleasedWithPendency" => Ok(Self::ReleasedWithPendency),
            "LeftForOperation" => Ok(Self::LeftForOperation),
            _ => Err(format!("Invalid GarageAttendanceStatus: {}", value)),
        }
    }
}

/// `EPIC-GA-02-S01` (`HRMS-958`, `TRM-414`): how a triage came to exist.
#[derive(Clone, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum GarageAttendanceOrigin {
    #[default]
    Manual,
    ArrivalAtBase,
}

impl Display for GarageAttendanceOrigin {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            GarageAttendanceOrigin::Manual => write!(f, "Manual"),
            GarageAttendanceOrigin::ArrivalAtBase => write!(f, "ArrivalAtBase"),
        }
    }
}

impl FromStr for GarageAttendanceOrigin {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Manual" => Ok(Self::Manual),
            "ArrivalAtBase" => Ok(Self::ArrivalAtBase),
            _ => Err(format!("Invalid GarageAttendanceOrigin: {}", value)),
        }
    }
}

/// `EPIC-GA-02-S02` (`HRMS-959`, `TRM-438`): the four states a service can be in.
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum GarageServiceState {
    #[default]
    Pending,
    Performed,
    NotNeeded,
    NotDone,
}

impl Display for GarageServiceState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            GarageServiceState::Pending => write!(f, "Pending"),
            GarageServiceState::Performed => write!(f, "Performed"),
            GarageServiceState::NotNeeded => write!(f, "NotNeeded"),
            GarageServiceState::NotDone => write!(f, "NotDone"),
        }
    }
}

impl FromStr for GarageServiceState {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Pending" => Ok(Self::Pending),
            "Performed" => Ok(Self::Performed),
            "NotNeeded" => Ok(Self::NotNeeded),
            "NotDone" => Ok(Self::NotDone),
            _ => Err(format!("Invalid GarageServiceState: {}", value)),
        }
    }
}

/// `EPIC-GA-03-S01` (`HRMS-960`, `TRM-770…773`): a physical arrival at the base
/// or departure from it.
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum PresenceEventKind {
    #[default]
    Arrival,
    Departure,
}

impl Display for PresenceEventKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PresenceEventKind::Arrival => write!(f, "Arrival"),
            PresenceEventKind::Departure => write!(f, "Departure"),
        }
    }
}

impl FromStr for PresenceEventKind {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Arrival" => Ok(Self::Arrival),
            "Departure" => Ok(Self::Departure),
            _ => Err(format!("Invalid presence event: {}", value)),
        }
    }
}

/// `EPIC-GA-03-S01` (`D-24(e)`, `TRM-788`): what a presence stamp was derived
/// from. Only a real tracker reading may stamp arrival or departure; a
/// schedule-derived estimate may set a display tag but never a stamp.
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default, Serialize, Deserialize, ToSchema)]
pub enum PresenceSource {
    #[default]
    Tracker,
    ScheduleEstimate,
}

impl FromStr for PresenceSource {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Tracker" => Ok(Self::Tracker),
            "ScheduleEstimate" => Ok(Self::ScheduleEstimate),
            _ => Err(format!("Invalid presence source: {}", value)),
        }
    }
}

/// `EPIC-SC-04-S01` (`HRMS-610`, `TRM-002`): the three kinds of transport demand
/// -- a recurring line, an `linha_extra` and a one-off trip (`viagem_avulsa`).
/// Stated by the owner on the demand, never inferred from its free-text
/// `demand_type` or its name: the garage reads "going to travel" (a one-off trip)
/// apart from a line or charter (`TRM-475`), and a guess there is a wrong alert.
#[derive(Clone, Copy, Eq, PartialEq, Debug, Serialize, Deserialize, ToSchema)]
pub enum DemandKind {
    Line,
    ExtraLine,
    OneOffTrip,
}

impl Display for DemandKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            DemandKind::Line => write!(f, "Line"),
            DemandKind::ExtraLine => write!(f, "ExtraLine"),
            DemandKind::OneOffTrip => write!(f, "OneOffTrip"),
        }
    }
}

impl FromStr for DemandKind {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Line" => Ok(Self::Line),
            "ExtraLine" => Ok(Self::ExtraLine),
            "OneOffTrip" => Ok(Self::OneOffTrip),
            _ => Err(format!("Invalid demand kind: {}", value)),
        }
    }
}
