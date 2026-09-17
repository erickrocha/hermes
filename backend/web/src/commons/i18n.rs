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

    /// A request for `pt-BR` (or bare `pt`) matches a supported `pt-BR`
    /// bundle; region is only compared when the request actually names one,
    /// so a bare `pt` still finds `pt-BR` rather than falling through to
    /// the `en` fallback.
    fn negotiates(supported: &LanguageIdentifier, requested: &LanguageIdentifier) -> bool {
        supported.language == requested.language
            && (requested.region.is_none() || supported.region == requested.region)
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
    PasswordChangeRequired,
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
            ErrorKey::PasswordChangeRequired => "PasswordChangeRequired",
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
            ErrorKey::PasswordChangeRequired => "password-change-required",
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
    use super::{translate, ErrorKey, Locale};
    use unic_langid::langid;

    #[test]
    fn falls_back_to_english_with_no_header() {
        assert_eq!(Locale::from_accept_language(None).language_id(), langid!("en"));
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
        assert_eq!(Locale::from_accept_language(Some("pt")).language_id(), langid!("pt-BR"));
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
        let en = translate(Locale::from_accept_language(Some("en")), ErrorKey::BadCredentials);
        let pt = translate(Locale::from_accept_language(Some("pt-BR")), ErrorKey::BadCredentials);
        assert_ne!(en, pt);
        assert!(!en.is_empty());
        assert!(!pt.is_empty());
    }
}
