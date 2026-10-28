use business::domain::enums::*;
use std::fmt::{Debug, Display};
use std::str::FromStr;

fn round_trip<E>(values: &[&str])
where
    E: FromStr<Err = String> + Display + PartialEq + Debug,
{
    for value in values {
        let parsed = E::from_str(value).unwrap();
        assert_eq!(parsed.to_string(), *value);
        assert_eq!(E::from_str(&parsed.to_string()).unwrap(), parsed);
    }
    assert!(E::from_str("not-a-domain-value").is_err());
}

fn parses<E>(values: &[&str])
where
    E: FromStr<Err = String> + PartialEq + Debug,
{
    for value in values {
        let first = E::from_str(value).unwrap();
        assert_eq!(E::from_str(value).unwrap(), first);
    }
    assert!(E::from_str("not-a-domain-value").is_err());
}

#[test]
fn persisted_enum_vocabularies_round_trip_every_variant() {
    round_trip::<ProfileScope>(&["Platform", "Tenant"]);
    round_trip::<VehicleStatus>(&["Active", "Maintenance", "Transit", "Reserved", "Inactive"]);
    round_trip::<GarageTagOrigin>(&["Manual", "Tracker", "Automatic"]);
    round_trip::<ScheduleExceptionType>(&["Cancellation", "Deallocation", "Substitution"]);
    round_trip::<TripStatus>(&["Scheduled", "Conflict", "PendingSchedule", "Cancelled"]);
    round_trip::<KmOrigin>(&[
        "InitialRegistration",
        "Manual",
        "WorkOrder",
        "DriverChecklist",
        "Garage",
        "TechnicalInspection",
        "Adjustment",
    ]);
    round_trip::<ChecklistType>(&["Departure", "Return", "Standalone"]);
    round_trip::<AnswerStatus>(&["Conforming", "NonConforming"]);
    round_trip::<WorkOrderStatus>(&[
        "Open",
        "PartiallyResolved",
        "AwaitingParts",
        "Concluded",
        "Cancelled",
    ]);
    round_trip::<WorkOrderItemStatus>(&["Pending", "Resolved", "Cancelled", "AwaitingParts"]);
    round_trip::<WorkOrderOrigin>(&["Manual", "Checklist", "Preventive", "Inspection"]);
    round_trip::<MaintenancePlanStatus>(&["Scheduled", "Concluded", "Cancelled", "NotExecuted"]);
    round_trip::<PurchaseOrderStatus>(&["Requested", "Ordered", "Purchased", "Cancelled"]);
    round_trip::<StockMovementType>(&["Entry", "Issue", "Adjustment"]);
    round_trip::<CostSource>(&[
        "Informed",
        "MovingAverageCost",
        "LastPurchasePrice",
        "RegisteredUnitValue",
        "None",
    ]);
    round_trip::<ExpenseOrigin>(&["Manual", "Import"]);
    round_trip::<FuelEntryOrigin>(&["Manual", "CtaSync", "DriverPhoto"]);
    round_trip::<PreventiveControlType>(&["Kilometers", "Days", "Both"]);
    round_trip::<GarageServiceGroup>(&["External", "Internal"]);
    round_trip::<GarageAttendanceStatus>(&[
        "Open",
        "Finished",
        "ReleasedWithPendency",
        "LeftForOperation",
    ]);
    round_trip::<GarageAttendanceOrigin>(&["Manual", "ArrivalAtBase"]);
    round_trip::<GarageServiceState>(&["Pending", "Performed", "NotNeeded", "NotDone"]);
    round_trip::<PresenceEventKind>(&["Arrival", "Departure"]);
    parses::<PresenceSource>(&["Tracker", "ScheduleEstimate"]);
    round_trip::<DemandKind>(&["Line", "ExtraLine", "OneOffTrip"]);
}
