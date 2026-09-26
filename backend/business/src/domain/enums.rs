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
}

impl Display for WorkOrderOrigin {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            WorkOrderOrigin::Manual => write!(f, "Manual"),
            WorkOrderOrigin::Checklist => write!(f, "Checklist"),
        }
    }
}

impl FromStr for WorkOrderOrigin {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "Manual" => Ok(Self::Manual),
            "Checklist" => Ok(Self::Checklist),
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
