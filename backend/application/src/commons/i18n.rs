use business::use_cases::extra_trip_import::ExtraTripImportError;
use business::use_cases::reference_import::ReferenceDataError;
use fluent_templates::{Loader, static_loader};
use unic_langid::{LanguageIdentifier, langid};

static_loader! {
    static LOCALES = {
        locales: "./locales",
        fallback_language: "en",
    };
}

/// EPIC-XF-06 (HRMS-031, HRMS-032, PD-022): the supported language set is
/// whatever bundles exist under `web/locales/` -- not a hardcoded pair.
/// Before this, adding a language meant a new enum variant and a new match
/// arm in both methods below; now it means a new
/// `web/locales/<tag>/errors.ftl` directory, and this resolution path
/// (`from_accept_language`, `language_id`, `translate` below) is not
/// touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Locale(LanguageIdentifier);

impl Locale {
    /// Picks the first requested language (in the header's own order --
    /// `;q=` weights are stripped, not used to re-sort; browsers already
    /// send the header in preference order in the common case, and full
    /// RFC 4647 negotiation is more than this platform needs today) that a
    /// bundle under `web/locales/` actually supports, falling back to `en`.
    pub fn from_accept_language(header: Option<&str>) -> Self {
        let requested = header
            .unwrap_or("en")
            .split(',')
            .filter_map(|part| part.split(';').next())
            .map(str::trim)
            .filter_map(|tag| tag.parse::<LanguageIdentifier>().ok());

        for candidate in requested {
            if let Some(supported) = LOCALES
                .locales()
                .find(|available| Self::negotiates(available, &candidate))
            {
                return Locale(supported.clone());
            }
        }

        Locale(langid!("en"))
    }

    /// Region is compared only when *both* sides name one. A bare `pt` finds
    /// the `pt-BR` bundle, and a regional `es-AR` finds the region-less `es`
    /// bundle.
    ///
    /// PD-031 surfaced the second half: the rule used to reject a request that
    /// named a region against a bundle that did not, so every Latin American
    /// browser sending `es-AR`, `es-MX` or `es-CO` fell through to English --
    /// precisely the readers Spanish was added for. `pt-PT` vs `pt-BR` still
    /// does not match: there both sides name a region, and they differ.
    fn negotiates(supported: &LanguageIdentifier, requested: &LanguageIdentifier) -> bool {
        supported.language == requested.language
            && (requested.region.is_none()
                || supported.region.is_none()
                || supported.region == requested.region)
    }

    pub fn language_id(&self) -> LanguageIdentifier {
        self.0.clone()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKey {
    AuthHeaderMissing,
    BadCredentials,
    InvalidCurrentPassword,
    RequiredParameterMissing,
    InvalidParameterValue,
    RequiredHeaderValueMissing,
    InvalidJwtToken,
    TenantCreatedFailed,
    TenantNotFound,
    TenantUpdateFailed,
    BusinessPlanForbidden,
    BusinessPlanNotFound,
    BusinessPlanInvalid,
    BusinessPlanInUse,
    CountryNotSupported,
    ReferenceDataUnavailable,
    /// DEF-RD-06: reference data is not a plan feature. Saying
    /// "business plan forbidden" to a TenantUser who tried to edit a city sent
    /// them to look at their subscription for a permission they will never buy.
    ReferenceDataForbidden,
    /// DEF-RD-06: a missing city or province is a missing *record*, not a
    /// missing request parameter.
    CityNotFound,
    ProvinceNotFound,
    /// DEF-BO-05: a role that may not create users. Distinct from a malformed
    /// request, which is what the caller used to be told.
    UserCreationForbidden,
    /// DEF-XF-02: a failure the caller cannot act on, but must be told about.
    /// Used where an empty result would otherwise be indistinguishable from a
    /// broken query.
    UnexpectedError,
    /// EPIC-FO-01-S05 (HRMS-924, PD-034): also what a caller outside the
    /// vehicle's tenant is told -- deliberately the same answer as a plate
    /// that was never registered.
    VehicleNotFound,
    /// HRMS-923: a role that may read the fleet but not change it.
    VehicleForbidden,
    /// `HRMS-600` (`C-024`): also what a caller outside the customer's
    /// tenant is told -- same shape as `VehicleNotFound`.
    CustomerNotFound,
    /// `HRMS-600`: a role that may read customers but not change them.
    CustomerForbidden,
    /// `HRMS-601` (`C-024`): this customer already has a day off recorded
    /// for that date (`uq_customer_day_off_customer_date`).
    DuplicateDayOff,
    /// `HRMS-601`: no such day off, or it belongs to another customer.
    DayOffNotFound,
    /// `HRMS-602` (`C-024`): a role that may not manage holidays.
    HolidayForbidden,
    /// `HRMS-602`: no such holiday, or it belongs to another tenant.
    HolidayNotFound,
    /// `HRMS-602`: this tenant already has a holiday recorded for that date
    /// (`uq_holiday_tenant_date`).
    DuplicateHoliday,
    /// `HRMS-603` (`C-024`): a role that may not record transport demand.
    TransportDemandForbidden,
    /// `HRMS-603`: no such demand, or it belongs to another tenant.
    TransportDemandNotFound,
    /// `HRMS-605` (`C-024`): this demand already has a schedule entry for
    /// that date (`uq_daily_schedule_demand_date`).
    DuplicateScheduleEntry,
    /// HRMS-922 (D-23(b)): a status outside the stated vocabulary.
    InvalidVehicleStatus,
    /// HRMS-941 (`C-023`): a garage tag origin outside `Manual`/`Tracker`/
    /// `Automatic`. Unlike `InvalidVehicleStatus`, an absent value is fine --
    /// this only fires when one was sent and not recognised.
    InvalidGarageTagOrigin,
    /// `HRMS-606` (`C-024`): an exception type outside `Cancellation`/
    /// `Deallocation`/`Substitution`. Required, so absent counts as unknown.
    InvalidScheduleExceptionType,
    /// `HRMS-607` (`C-024`, `D-24(f)`): a trip status outside `Scheduled`/
    /// `Conflict`/`PendingSchedule`/`Cancelled`.
    InvalidTripStatus,
    /// `HRMS-607`/`D-24(d)`: this tenant already has a trip with that order
    /// code on that date (`uq_extra_trip_tenant_code_date`).
    DuplicateTrip,
    /// `HRMS-607`: a role that may not record trips.
    ExtraTripForbidden,
    /// `HRMS-607`: no such trip, or it belongs to another tenant.
    ExtraTripNotFound,
    /// `HRMS-650` (`C-025`, `TRM-151`): an origin outside the seven-value
    /// kilometre vocabulary. Required, so absent counts as unknown.
    InvalidKmOrigin,
    /// `HRMS-651` (`C-025`): a checklist type outside `Departure`/`Return`/
    /// `Standalone`. Required, so absent counts as unknown.
    InvalidChecklistType,
    /// `HRMS-651`: a role that may not manage checklist templates.
    ChecklistTemplateForbidden,
    /// `HRMS-651`: no such template, or it belongs to another tenant.
    ChecklistTemplateNotFound,
    /// `HRMS-652`: a role that may not submit checklist runs.
    ChecklistRunForbidden,
    /// `HRMS-652`: no such checklist run, or it belongs to another tenant.
    ChecklistRunNotFound,
    /// `HRMS-654` (`C-025`, `TRM-115`/`TRM-116`): an answer status outside
    /// `Conforming`/`NonConforming`. Required, so absent counts as unknown.
    InvalidAnswerStatus,
    /// `HRMS-700`: a role that may not open work orders.
    WorkOrderForbidden,
    /// `HRMS-700`: no such work order, or it belongs to another tenant.
    WorkOrderNotFound,
    /// `HRMS-701` (`C-026`, `TRM-203`): an item status outside
    /// `Pending`/`Resolved`/`Cancelled`/`AwaitingParts`. Required, so
    /// absent counts as unknown.
    InvalidWorkOrderItemStatus,
    /// `HRMS-703`: a role that may not schedule maintenance plans.
    MaintenancePlanForbidden,
    /// `HRMS-703`: no such maintenance plan, or it belongs to another tenant.
    MaintenancePlanNotFound,
    /// `HRMS-704`: a role that may not manage the service-type catalogue.
    ServiceTypeForbidden,
    /// `HRMS-704`: no such service type, or it belongs to another tenant.
    ServiceTypeNotFound,
    /// `HRMS-704`: this tenant already has a service type with this code.
    DuplicateServiceCode,
    /// `HRMS-704`: a role that may not manage the priced-service catalogue.
    PricedServiceForbidden,
    /// `HRMS-704`: no such priced service, or it belongs to another tenant.
    PricedServiceNotFound,
    /// HRMS-925 (D-23(c)): this plate is already registered in the caller's
    /// own tenant. Never raised for another tenant's vehicle.
    DuplicatePlate,
    /// HRMS-926: only the platform administrator links a tracking device.
    TrackerLinkForbidden,
    /// HRMS-926: the device already reports for another vehicle.
    DuplicateTrackerDevice,
    /// HRMS-928: the vehicle has no tracking device, so there is no position to give.
    TrackerNotLinked,
    /// HRMS-928: the tracking provider is unconfigured or did not answer.
    TrackingUnavailable,
    /// D-23(d): the vehicle already has a live assignment.
    VehicleAlreadyAssigned,
    /// HRMS-933: the vehicle has no live assignment.
    NoLiveAssignment,
    /// HRMS-932: not an active driver of the vehicle's tenant.
    NotADriver,
}

impl ErrorKey {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorKey::AuthHeaderMissing => "AuthHeaderMissing",
            ErrorKey::BadCredentials => "BadCredentials",
            ErrorKey::InvalidCurrentPassword => "InvalidCurrentPassword",
            ErrorKey::RequiredParameterMissing => "RequiredParameterMissing",
            ErrorKey::InvalidParameterValue => "InvalidParameterValue",
            ErrorKey::RequiredHeaderValueMissing => "RequiredHeaderValueMissing",
            ErrorKey::InvalidJwtToken => "InvalidJwtToken",
            ErrorKey::TenantCreatedFailed => "TenantCreatedFailed",
            ErrorKey::TenantNotFound => "TenantNotFound",
            ErrorKey::TenantUpdateFailed => "TenantUpdateFailed",
            ErrorKey::BusinessPlanForbidden => "BusinessPlanForbidden",
            ErrorKey::BusinessPlanNotFound => "BusinessPlanNotFound",
            ErrorKey::BusinessPlanInvalid => "BusinessPlanInvalid",
            ErrorKey::BusinessPlanInUse => "BusinessPlanInUse",
            ErrorKey::CountryNotSupported => "CountryNotSupported",
            ErrorKey::ReferenceDataUnavailable => "ReferenceDataUnavailable",
            ErrorKey::ReferenceDataForbidden => "ReferenceDataForbidden",
            ErrorKey::CityNotFound => "CityNotFound",
            ErrorKey::ProvinceNotFound => "ProvinceNotFound",
            ErrorKey::UserCreationForbidden => "UserCreationForbidden",
            ErrorKey::UnexpectedError => "UnexpectedError",
            ErrorKey::VehicleNotFound => "VehicleNotFound",
            ErrorKey::VehicleForbidden => "VehicleForbidden",
            ErrorKey::CustomerNotFound => "CustomerNotFound",
            ErrorKey::CustomerForbidden => "CustomerForbidden",
            ErrorKey::DuplicateDayOff => "DuplicateDayOff",
            ErrorKey::DayOffNotFound => "DayOffNotFound",
            ErrorKey::HolidayForbidden => "HolidayForbidden",
            ErrorKey::HolidayNotFound => "HolidayNotFound",
            ErrorKey::DuplicateHoliday => "DuplicateHoliday",
            ErrorKey::TransportDemandForbidden => "TransportDemandForbidden",
            ErrorKey::TransportDemandNotFound => "TransportDemandNotFound",
            ErrorKey::DuplicateScheduleEntry => "DuplicateScheduleEntry",
            ErrorKey::InvalidVehicleStatus => "InvalidVehicleStatus",
            ErrorKey::InvalidGarageTagOrigin => "InvalidGarageTagOrigin",
            ErrorKey::InvalidScheduleExceptionType => "InvalidScheduleExceptionType",
            ErrorKey::InvalidTripStatus => "InvalidTripStatus",
            ErrorKey::DuplicateTrip => "DuplicateTrip",
            ErrorKey::ExtraTripForbidden => "ExtraTripForbidden",
            ErrorKey::ExtraTripNotFound => "ExtraTripNotFound",
            ErrorKey::InvalidKmOrigin => "InvalidKmOrigin",
            ErrorKey::InvalidChecklistType => "InvalidChecklistType",
            ErrorKey::ChecklistTemplateForbidden => "ChecklistTemplateForbidden",
            ErrorKey::ChecklistTemplateNotFound => "ChecklistTemplateNotFound",
            ErrorKey::ChecklistRunForbidden => "ChecklistRunForbidden",
            ErrorKey::ChecklistRunNotFound => "ChecklistRunNotFound",
            ErrorKey::InvalidAnswerStatus => "InvalidAnswerStatus",
            ErrorKey::WorkOrderForbidden => "WorkOrderForbidden",
            ErrorKey::WorkOrderNotFound => "WorkOrderNotFound",
            ErrorKey::InvalidWorkOrderItemStatus => "InvalidWorkOrderItemStatus",
            ErrorKey::MaintenancePlanForbidden => "MaintenancePlanForbidden",
            ErrorKey::MaintenancePlanNotFound => "MaintenancePlanNotFound",
            ErrorKey::ServiceTypeForbidden => "ServiceTypeForbidden",
            ErrorKey::ServiceTypeNotFound => "ServiceTypeNotFound",
            ErrorKey::DuplicateServiceCode => "DuplicateServiceCode",
            ErrorKey::PricedServiceForbidden => "PricedServiceForbidden",
            ErrorKey::PricedServiceNotFound => "PricedServiceNotFound",
            ErrorKey::DuplicatePlate => "DuplicatePlate",
            ErrorKey::TrackerLinkForbidden => "TrackerLinkForbidden",
            ErrorKey::DuplicateTrackerDevice => "DuplicateTrackerDevice",
            ErrorKey::TrackerNotLinked => "TrackerNotLinked",
            ErrorKey::TrackingUnavailable => "TrackingUnavailable",
            ErrorKey::VehicleAlreadyAssigned => "VehicleAlreadyAssigned",
            ErrorKey::NoLiveAssignment => "NoLiveAssignment",
            ErrorKey::NotADriver => "NotADriver",
        }
    }

    pub fn message_id(self) -> &'static str {
        match self {
            ErrorKey::AuthHeaderMissing => "auth-header-missing",
            ErrorKey::BadCredentials => "bad-credentials",
            ErrorKey::InvalidCurrentPassword => "invalid-current-password",
            ErrorKey::RequiredParameterMissing => "required-parameter-missing",
            ErrorKey::InvalidParameterValue => "invalid-parameter-value",
            ErrorKey::RequiredHeaderValueMissing => "required-header-value-missing",
            ErrorKey::InvalidJwtToken => "invalid-jwt-token",
            ErrorKey::TenantCreatedFailed => "tenant-created-failed",
            ErrorKey::TenantNotFound => "tenant-not-found",
            ErrorKey::TenantUpdateFailed => "tenant-update-failed",
            ErrorKey::BusinessPlanForbidden => "business-plan-forbidden",
            ErrorKey::BusinessPlanNotFound => "business-plan-not-found",
            ErrorKey::BusinessPlanInvalid => "business-plan-invalid",
            ErrorKey::BusinessPlanInUse => "business-plan-in-use",
            ErrorKey::CountryNotSupported => "country-not-supported",
            ErrorKey::ReferenceDataUnavailable => "reference-data-unavailable",
            ErrorKey::ReferenceDataForbidden => "reference-data-forbidden",
            ErrorKey::CityNotFound => "city-not-found",
            ErrorKey::ProvinceNotFound => "province-not-found",
            ErrorKey::UserCreationForbidden => "user-creation-forbidden",
            ErrorKey::UnexpectedError => "unexpected-error",
            ErrorKey::VehicleNotFound => "vehicle-not-found",
            ErrorKey::VehicleForbidden => "vehicle-forbidden",
            ErrorKey::CustomerNotFound => "customer-not-found",
            ErrorKey::CustomerForbidden => "customer-forbidden",
            ErrorKey::DuplicateDayOff => "duplicate-day-off",
            ErrorKey::DayOffNotFound => "day-off-not-found",
            ErrorKey::HolidayForbidden => "holiday-forbidden",
            ErrorKey::HolidayNotFound => "holiday-not-found",
            ErrorKey::DuplicateHoliday => "duplicate-holiday",
            ErrorKey::TransportDemandForbidden => "transport-demand-forbidden",
            ErrorKey::TransportDemandNotFound => "transport-demand-not-found",
            ErrorKey::DuplicateScheduleEntry => "duplicate-schedule-entry",
            ErrorKey::InvalidVehicleStatus => "invalid-vehicle-status",
            ErrorKey::InvalidGarageTagOrigin => "invalid-garage-tag-origin",
            ErrorKey::InvalidScheduleExceptionType => "invalid-schedule-exception-type",
            ErrorKey::InvalidTripStatus => "invalid-trip-status",
            ErrorKey::DuplicateTrip => "duplicate-trip",
            ErrorKey::ExtraTripForbidden => "extra-trip-forbidden",
            ErrorKey::ExtraTripNotFound => "extra-trip-not-found",
            ErrorKey::InvalidKmOrigin => "invalid-km-origin",
            ErrorKey::InvalidChecklistType => "invalid-checklist-type",
            ErrorKey::ChecklistTemplateForbidden => "checklist-template-forbidden",
            ErrorKey::ChecklistTemplateNotFound => "checklist-template-not-found",
            ErrorKey::ChecklistRunForbidden => "checklist-run-forbidden",
            ErrorKey::ChecklistRunNotFound => "checklist-run-not-found",
            ErrorKey::InvalidAnswerStatus => "invalid-answer-status",
            ErrorKey::WorkOrderForbidden => "work-order-forbidden",
            ErrorKey::WorkOrderNotFound => "work-order-not-found",
            ErrorKey::InvalidWorkOrderItemStatus => "invalid-work-order-item-status",
            ErrorKey::MaintenancePlanForbidden => "maintenance-plan-forbidden",
            ErrorKey::MaintenancePlanNotFound => "maintenance-plan-not-found",
            ErrorKey::ServiceTypeForbidden => "service-type-forbidden",
            ErrorKey::ServiceTypeNotFound => "service-type-not-found",
            ErrorKey::DuplicateServiceCode => "duplicate-service-code",
            ErrorKey::PricedServiceForbidden => "priced-service-forbidden",
            ErrorKey::PricedServiceNotFound => "priced-service-not-found",
            ErrorKey::DuplicatePlate => "duplicate-plate",
            ErrorKey::TrackerLinkForbidden => "tracker-link-forbidden",
            ErrorKey::DuplicateTrackerDevice => "duplicate-tracker-device",
            ErrorKey::TrackerNotLinked => "tracker-not-linked",
            ErrorKey::TrackingUnavailable => "tracking-unavailable",
            ErrorKey::VehicleAlreadyAssigned => "vehicle-already-assigned",
            ErrorKey::NoLiveAssignment => "no-live-assignment",
            ErrorKey::NotADriver => "not-a-driver",
        }
    }
}

/// Resolves a Fluent message id that is not an `ErrorKey`.
///
/// DEF-RD-04: the import rejection reasons are owned by the `business` crate,
/// which has no locale and no `ErrorKey`. It names a reason
/// (`RejectionReason::message_id`) and this is where that name becomes a
/// sentence in the caller's language.
pub fn translate_id(locale: &Locale, message_id: &str) -> String {
    let lang_id = locale.language_id();
    match LOCALES.try_lookup(&lang_id, message_id) {
        Some(message) => message,
        None => {
            log::error!("missing fluent translation: locale={lang_id} key={message_id}");
            message_id.to_string()
        }
    }
}

/// Renders an import failure in the caller's language.
///
/// DEF-RD-04: every part is translated -- the heading, the word "row" and each
/// reason. Only the row number is formatted here, because a number is the same
/// in all three bundles. Before this the whole sentence was built in English in
/// the business layer, so a Portuguese operator read English rejections under
/// Portuguese column headings.
pub fn translate_reference_data_error(locale: &Locale, error: &ReferenceDataError) -> String {
    match error {
        ReferenceDataError::Unavailable => {
            translate(locale.clone(), ErrorKey::ReferenceDataUnavailable)
        }
        ReferenceDataError::Rejected(rejections) => {
            let row_word = translate_id(locale, "import-row");
            let detail = rejections
                .iter()
                .map(|rejection| {
                    format!(
                        "{} {}: {}",
                        row_word,
                        rejection.row + 1,
                        translate_id(locale, rejection.reason.message_id())
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            format!("{} {}", translate_id(locale, "import-rejected"), detail)
        }
    }
}

/// `EPIC-SC-03-S02` (`HRMS-608`): same shape as `translate_reference_data_
/// error`, for the trip import's own rejection reasons.
pub fn translate_extra_trip_import_error(locale: &Locale, error: &ExtraTripImportError) -> String {
    match error {
        ExtraTripImportError::Unavailable => translate(locale.clone(), ErrorKey::UnexpectedError),
        ExtraTripImportError::Rejected(rejections) => {
            let row_word = translate_id(locale, "import-row");
            let detail = rejections
                .iter()
                .map(|rejection| {
                    format!(
                        "{} {}: {}",
                        row_word,
                        rejection.row + 1,
                        translate_id(locale, rejection.reason.message_id())
                    )
                })
                .collect::<Vec<_>>()
                .join("; ");
            format!("{} {}", translate_id(locale, "import-rejected"), detail)
        }
    }
}

pub fn translate(locale: Locale, key: ErrorKey) -> String {
    let lang_id = locale.language_id();
    match LOCALES.try_lookup(&lang_id, key.message_id()) {
        Some(message) => message,
        None => {
            log::error!(
                "missing fluent translation: locale={} key={}",
                lang_id,
                key.message_id()
            );
            "An unexpected error occurred".to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ErrorKey, Locale, translate};
    use unic_langid::langid;

    #[test]
    fn falls_back_to_english_with_no_header() {
        assert_eq!(
            Locale::from_accept_language(None).language_id(),
            langid!("en")
        );
    }

    #[test]
    fn resolves_an_exact_supported_tag() {
        assert_eq!(
            Locale::from_accept_language(Some("pt-BR")).language_id(),
            langid!("pt-BR")
        );
    }

    #[test]
    fn a_bare_language_without_region_still_finds_the_regional_bundle() {
        // EPIC-XF-06's negotiation is deliberately looser than an exact
        // match: a request for plain "pt" should still find the "pt-BR"
        // bundle rather than falling through to "en".
        assert_eq!(
            Locale::from_accept_language(Some("pt")).language_id(),
            langid!("pt-BR")
        );
    }

    #[test]
    fn picks_the_first_supported_language_in_header_order() {
        // "fr-FR" has no bundle; "pt-BR" does, and comes second.
        assert_eq!(
            Locale::from_accept_language(Some("fr-FR,pt-BR;q=0.8,en;q=0.5")).language_id(),
            langid!("pt-BR")
        );
    }

    #[test]
    fn falls_back_to_english_when_nothing_requested_is_supported() {
        assert_eq!(
            Locale::from_accept_language(Some("fr-FR,de-DE")).language_id(),
            langid!("en")
        );
    }

    #[test]
    fn falls_back_to_english_on_a_malformed_header_without_panicking() {
        assert_eq!(
            Locale::from_accept_language(Some(",,;;garbage===")).language_id(),
            langid!("en")
        );
    }

    #[test]
    fn translate_resolves_a_real_message_in_the_requested_language() {
        let en = translate(
            Locale::from_accept_language(Some("en")),
            ErrorKey::BadCredentials,
        );
        let pt = translate(
            Locale::from_accept_language(Some("pt-BR")),
            ErrorKey::BadCredentials,
        );
        assert_ne!(en, pt);
        assert!(!en.is_empty());
        assert!(!pt.is_empty());
    }

    #[test]
    fn spanish_is_a_supported_bundle() {
        // PD-031: es junta-se a en e pt-BR. Nada no código precisou mudar --
        // o conjunto suportado é o que existe em `web/locales/`; este teste
        // existe para que remover o bundle seja uma falha, não um silêncio.
        assert_eq!(
            Locale::from_accept_language(Some("es")).language_id(),
            langid!("es")
        );
        // Um bundle sem região atende qualquer região: es-AR/es-MX/es-CO são
        // exatamente os leitores para quem o espanhol foi adicionado.
        assert_eq!(
            Locale::from_accept_language(Some("es-AR")).language_id(),
            langid!("es")
        );
        assert_eq!(
            Locale::from_accept_language(Some("es-MX")).language_id(),
            langid!("es")
        );
    }

    /// EPIC-FO-01 (HRMS-922/924/925): the fleet keys exist in every bundle.
    /// A missing one does not fail the request -- `translate` falls back to a
    /// generic English sentence and logs -- so nothing but a test notices that
    /// a Portuguese or Spanish reader is being told the wrong thing. Extended
    /// 2026-09-25 to the customer registry's keys (`HRMS-600`, `C-024`) --
    /// same shape, same reason, not worth a second near-identical test.
    #[test]
    fn every_vehicle_error_is_worded_in_every_bundle() {
        let keys = [
            ErrorKey::VehicleNotFound,
            ErrorKey::VehicleForbidden,
            ErrorKey::InvalidVehicleStatus,
            ErrorKey::InvalidGarageTagOrigin,
            ErrorKey::DuplicatePlate,
            ErrorKey::TrackerLinkForbidden,
            ErrorKey::DuplicateTrackerDevice,
            ErrorKey::TrackerNotLinked,
            ErrorKey::TrackingUnavailable,
            ErrorKey::VehicleAlreadyAssigned,
            ErrorKey::NoLiveAssignment,
            ErrorKey::NotADriver,
            ErrorKey::CustomerNotFound,
            ErrorKey::CustomerForbidden,
            ErrorKey::DuplicateDayOff,
            ErrorKey::DayOffNotFound,
            ErrorKey::HolidayForbidden,
            ErrorKey::HolidayNotFound,
            ErrorKey::DuplicateHoliday,
            ErrorKey::TransportDemandForbidden,
            ErrorKey::TransportDemandNotFound,
            ErrorKey::DuplicateScheduleEntry,
            ErrorKey::InvalidScheduleExceptionType,
            ErrorKey::InvalidTripStatus,
            ErrorKey::DuplicateTrip,
            ErrorKey::ExtraTripForbidden,
            ErrorKey::ExtraTripNotFound,
            ErrorKey::InvalidKmOrigin,
            ErrorKey::InvalidChecklistType,
            ErrorKey::ChecklistTemplateForbidden,
            ErrorKey::ChecklistTemplateNotFound,
            ErrorKey::ChecklistRunForbidden,
            ErrorKey::ChecklistRunNotFound,
            ErrorKey::InvalidAnswerStatus,
            ErrorKey::WorkOrderForbidden,
            ErrorKey::WorkOrderNotFound,
            ErrorKey::InvalidWorkOrderItemStatus,
            ErrorKey::MaintenancePlanForbidden,
            ErrorKey::MaintenancePlanNotFound,
            ErrorKey::ServiceTypeForbidden,
            ErrorKey::ServiceTypeNotFound,
            ErrorKey::DuplicateServiceCode,
            ErrorKey::PricedServiceForbidden,
            ErrorKey::PricedServiceNotFound,
        ];
        let generic = "An unexpected error occurred";
        for tag in ["en", "pt-BR", "es"] {
            let locale = Locale::from_accept_language(Some(tag));
            for key in keys {
                let message = translate(locale.clone(), key);
                assert_ne!(message, generic, "{tag} is missing {}", key.message_id());
                assert!(!message.is_empty());
            }
        }

        // And they are actually translated, not the English text copied over.
        assert_ne!(
            translate(
                Locale::from_accept_language(Some("en")),
                ErrorKey::DuplicatePlate
            ),
            translate(
                Locale::from_accept_language(Some("pt-BR")),
                ErrorKey::DuplicatePlate
            )
        );
    }

    #[test]
    fn a_differing_region_on_both_sides_still_does_not_match() {
        // pt-PT não é atendido pelo bundle pt-BR; cai no fallback.
        assert_eq!(
            Locale::from_accept_language(Some("pt-PT")).language_id(),
            langid!("en")
        );
    }

    /// DEF-RD-04: the rejection reasons are named by the `business` crate and
    /// worded here. A reason with no message in some bundle would reach that
    /// reader as a bare message id, which is how the English-under-Portuguese
    /// defect looked in the first place.
    #[test]
    fn every_rejection_reason_is_worded_in_every_bundle() {
        use business::use_cases::reference_import::RejectionReason::*;
        let reasons = [
            NameRequired,
            NameTooLong,
            ProvinceRequired,
            ProvinceNotFound,
            AcronymRequired,
            AcronymTooLong,
            CountryCodeInvalid,
            DuplicateCityInFile,
            DuplicateAcronymInFile,
            CityAlreadyExists,
            ProvinceAlreadyExists,
        ];
        for tag in ["en", "pt-BR", "es"] {
            let locale = Locale::from_accept_language(Some(tag));
            for id in ["import-rejected", "import-row"] {
                assert_ne!(
                    super::translate_id(&locale, id),
                    id,
                    "{tag} is missing {id}"
                );
            }
            for reason in reasons {
                let id = reason.message_id();
                assert_ne!(
                    super::translate_id(&locale, id),
                    id,
                    "{tag} is missing {id}"
                );
            }
        }
    }

    #[test]
    fn a_rejected_import_is_reported_in_the_readers_language() {
        use business::use_cases::reference_import::{
            ImportRejection, ReferenceDataError, RejectionReason,
        };
        let error = ReferenceDataError::Rejected(vec![
            ImportRejection::new(0, RejectionReason::NameRequired),
            ImportRejection::new(2, RejectionReason::ProvinceNotFound),
        ]);
        let en = super::translate_reference_data_error(
            &Locale::from_accept_language(Some("en")),
            &error,
        );
        let pt = super::translate_reference_data_error(
            &Locale::from_accept_language(Some("pt-BR")),
            &error,
        );

        // The row numbers are the operator's, one-based, in both languages.
        assert!(en.contains("row 1"), "{en}");
        assert!(en.contains("row 3"), "{en}");
        assert!(pt.contains("linha 1"), "{pt}");
        assert!(pt.contains("linha 3"), "{pt}");
        assert_ne!(
            en, pt,
            "a Portuguese reader must not be shown the English wording"
        );
    }

    #[test]
    fn an_unavailable_database_is_never_described_to_the_caller() {
        use business::use_cases::reference_import::ReferenceDataError;
        let message = super::translate_reference_data_error(
            &Locale::from_accept_language(Some("en")),
            &ReferenceDataError::Unavailable,
        );
        assert_eq!(
            message,
            translate(
                Locale::from_accept_language(Some("en")),
                ErrorKey::ReferenceDataUnavailable
            )
        );
    }
}
