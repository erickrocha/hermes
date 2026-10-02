//! `EPIC-SC-03-S02` (`HRMS-608`, `D-24(d)`): bulk import of extra trips,
//! reviewed on screen before saving (`PD-027`) -- the same all-or-nothing
//! discipline `reference_import.rs` established for city/province, applied
//! to this domain's own identity rule: order code **and** date, not code
//! alone (`U-272`).
//!
//! A separate error type from `reference_import::ReferenceDataError`
//! rather than a generalisation of it: the rejection reasons are entirely
//! different (driver/vehicle/customer tenant checks have no city/province
//! analogue), and forcing one generic type over both would only replace a
//! short, honest duplication with an indirection that serves neither caller
//! well.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TripRejectionReason {
    OrderCodeRequired,
    /// Same tenant, same order code, same date as an earlier row in this
    /// file -- `D-24(d)`'s identity pair, checked within the batch the same
    /// way `find_duplicate_keys` already checks city/province imports.
    DuplicateTripInFile,
    NotADriver,
    NotATenantVehicle,
    NotATenantCustomer,
}

impl TripRejectionReason {
    /// The Fluent message id the HTTP layer looks up.
    pub fn message_id(self) -> &'static str {
        match self {
            TripRejectionReason::OrderCodeRequired => "import-trip-order-code-required",
            TripRejectionReason::DuplicateTripInFile => "import-trip-duplicate-in-file",
            TripRejectionReason::NotADriver => "import-trip-not-a-driver",
            TripRejectionReason::NotATenantVehicle => "import-trip-not-a-tenant-vehicle",
            TripRejectionReason::NotATenantCustomer => "import-trip-not-a-tenant-customer",
        }
    }

    /// Used in logs, and by any caller that has no locale to hand.
    pub fn english(self) -> &'static str {
        match self {
            TripRejectionReason::OrderCodeRequired => "order code is required",
            TripRejectionReason::DuplicateTripInFile => {
                "duplicate order code and date within the file"
            }
            TripRejectionReason::NotADriver => "not an active driver of this tenant",
            TripRejectionReason::NotATenantVehicle => "vehicle does not belong to this tenant",
            TripRejectionReason::NotATenantCustomer => "customer does not belong to this tenant",
        }
    }
}

/// `row` is the base-zero index into the data the caller sent, the same
/// convention `reference_import::ImportRejection` uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TripImportRejection {
    pub row: usize,
    pub reason: TripRejectionReason,
}

impl TripImportRejection {
    pub fn new(row: usize, reason: TripRejectionReason) -> Self {
        Self { row, reason }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtraTripImportError {
    Rejected(Vec<TripImportRejection>),
    Unavailable,
}

impl ExtraTripImportError {
    pub fn unavailable(context: &str, error: impl std::fmt::Display) -> Self {
        log::error!("[{context}] Extra trip import failed: {error}");
        ExtraTripImportError::Unavailable
    }

    /// The English rendering, for logs and for tests.
    pub fn english(&self) -> String {
        match self {
            ExtraTripImportError::Unavailable => "Trip import is temporarily unavailable".to_string(),
            ExtraTripImportError::Rejected(rejections) => {
                let detail = rejections
                    .iter()
                    .map(|r| format!("row {}: {}", r.row + 1, r.reason.english()))
                    .collect::<Vec<_>>()
                    .join("; ");
                format!("Import rejected, nothing was written. {detail}")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ExtraTripImportError, TripImportRejection, TripRejectionReason};

    #[test]
    fn every_reason_has_a_distinct_message_id() {
        let reasons = [
            TripRejectionReason::OrderCodeRequired,
            TripRejectionReason::DuplicateTripInFile,
            TripRejectionReason::NotADriver,
            TripRejectionReason::NotATenantVehicle,
            TripRejectionReason::NotATenantCustomer,
        ];
        let ids: std::collections::HashSet<_> = reasons.iter().map(|r| r.message_id()).collect();
        assert_eq!(ids.len(), reasons.len(), "message ids must be unique");
        assert!(reasons.iter().all(|r| !r.english().is_empty()));
    }

    #[test]
    fn the_error_numbers_rows_the_way_the_operator_sees_them() {
        let error = ExtraTripImportError::Rejected(vec![
            TripImportRejection::new(0, TripRejectionReason::OrderCodeRequired),
            TripImportRejection::new(3, TripRejectionReason::DuplicateTripInFile),
        ]);
        let message = error.english();
        assert!(message.contains("row 1: order code is required"), "{message}");
        assert!(
            message.contains("row 4: duplicate order code and date within the file"),
            "{message}"
        );
        assert!(message.contains("nothing was written"));
    }

    #[test]
    fn an_unavailable_write_never_discloses_the_database() {
        let error = ExtraTripImportError::unavailable(
            "test",
            "Execution Error: 1452 (23000): Cannot add or update a child row: a foreign key constraint fails (`hermes`.`extra_trip`)",
        );
        let message = error.english();
        assert!(!message.contains("1452"), "{message}");
        assert!(!message.contains("hermes"), "{message}");
    }
}
