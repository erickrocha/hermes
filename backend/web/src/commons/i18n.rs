use fluent_templates::{Loader, static_loader};
use unic_langid::{LanguageIdentifier, langid};

static_loader! {
    static LOCALES = {
        locales: "./locales",
        fallback_language: "en",
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    En,
    PtBr,
}

impl Locale {
    pub fn from_accept_language(header: Option<&str>) -> Self {
        let lang = header
            .unwrap_or("en")
            .split(',')
            .next()
            .unwrap_or("en")
            .trim()
            .to_ascii_lowercase();

        if lang.starts_with("pt-br") {
            Locale::PtBr
        } else {
            Locale::En
        }
    }

    pub fn language_id(self) -> LanguageIdentifier {
        match self {
            Locale::En => langid!("en"),
            Locale::PtBr => langid!("pt-BR"),
        }
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
