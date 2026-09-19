use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::commons::tax_id::{self, TaxIdOutcome};
use crate::domain::business_error::BusinessError;
use crate::domain::tenant::{Tenant, TenantEntityMapper};
use crate::gateway::tenant_gateway::TenantGateway;

/// EPIC-TP-01-S04 (HRMS-203, PD-022): country is now a required part of a
/// tenant's record, not optional -- both tax-identifier validation
/// (EPIC-TP-02) and address reference data (EPIC-RD-01) are selected by it,
/// so a tenant without one can't be either.
fn valid_country_code(value: &Option<String>) -> bool {
    value.as_ref().is_some_and(|code| code.len() == 2 && code.chars().all(|c| c.is_ascii_alphabetic()))
}

/// EPIC-TP-01 (HRMS-201/HRM-001): the business name identifies the tenant, so
/// it is required in substance and not just in form — a name made of spaces is
/// the "list of blanks" the story exists to prevent (DEF-TP-03).
fn valid_business_name(value: &str) -> bool {
    !value.trim().is_empty()
}

/// EPIC-TP-02 (HRMS-207...211, PD-022): validates and normalises `tax_id`
/// against whichever country the tenant is being saved with. A country with
/// no registered validator (S05) is accepted as given -- expanding into a
/// new market must not be blocked on writing that country's validator
/// first, and must not have another country's rules silently applied to it.
fn validate_tax_id(country_code: &str, tax_id: &str) -> Result<String, BusinessError> {
    // HRMS-200/HRM-001: the tax identifier is a required element of a tenant.
    // Without this check every country that has no validator yet accepted a
    // tenant with no legal identity at all (DEF-TP-01) — and because the
    // identifier is unique per country (HRMS-211), the empty string then
    // counted as a value and collided with the next such tenant.
    if tax_id.trim().is_empty() {
        return Err(BusinessError::new("Tax identifier is required".to_string()));
    }

    match tax_id::validate_and_normalize(country_code, tax_id) {
        TaxIdOutcome::Valid(normalized) => Ok(normalized),
        TaxIdOutcome::Invalid => Err(BusinessError::new(format!(
            "Tax identifier is not a valid document for country {country_code}"
        ))),
        TaxIdOutcome::Unsupported => Ok(tax_id.trim().to_string()),
    }
}

pub struct TenantUseCase {
    gateway: TenantGateway,
}

impl TenantUseCase {
    pub fn new(gateway: TenantGateway) -> Self {
        Self { gateway }
    }

    pub async fn create(&self, tenant: Tenant) -> Result<Tenant, BusinessError> {
        log::info!("[TenantUseCase::create] Executing create tenant for business name: {:?}", tenant.business_name);

        if !valid_business_name(&tenant.business_name) {
            let msg = "Tenant business name is required".to_string();
            log::error!("[TenantUseCase::create] {}", msg);
            return Err(BusinessError::new(msg));
        }
        if !valid_country_code(&tenant.country_code) {
            return Err(BusinessError::new("Country code must contain two letters".to_string()));
        }
        // valid_country_code above guarantees Some at this point.
        let country_code = tenant.country_code.clone().unwrap();
        let tax_id = validate_tax_id(&country_code, &tenant.tax_id)?;

        // A tenant's plan is set only through `set_plan` (HRMS-224, PD-021),
        // never at creation, regardless of what the caller sent.
        let tenant = Tenant { business_plan_id: None, tax_id, ..tenant };

        let entity = self
            .gateway
            .persist(tenant)
            .await
            .map_err(|e| {
                let msg = format!("Failed to persist tenant: {}", e);
                log::error!("[TenantUseCase::create] {}", msg);
                BusinessError::new(msg)
            })?;

        Ok(TenantEntityMapper::from_active_model(entity))
    }

    pub async fn find_by_id(&self, id: i64) -> Result<Tenant, BusinessError> {
        log::info!("[TenantUseCase::find_by_id] Executing for id: {}", id);

        let entity = self
            .gateway
            .find_by_id(id)
            .await
            .map_err(|e| {
                let msg = format!("Database error: {}", e);
                log::error!("[TenantUseCase::find_by_id] {}", msg);
                BusinessError::new(msg)
            })?;

        match entity {
            Some(value) => Ok(TenantEntityMapper::from_model(value)),
            None => {
                let msg = format!("Tenant not found with id: {}", id);
                log::error!("[TenantUseCase::find_by_id] {}", msg);
                Err(BusinessError::not_found("Tenant not found".to_string()))
            }
        }
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<Tenant, BusinessError> {
        log::info!("[TenantUseCase::find_by_uuid] Executing for uuid: {}", uuid);

        let entity = self
            .gateway
            .find_by_uuid(uuid.clone())
            .await
            .map_err(|e| {
                let msg = format!("Database error: {}", e);
                log::error!("[TenantUseCase::find_by_uuid] {}", msg);
                BusinessError::new(msg)
            })?;

        match entity {
            Some(value) => Ok(TenantEntityMapper::from_model(value)),
            None => {
                let msg = format!("Tenant not found with uuid: {}", uuid);
                log::error!("[TenantUseCase::find_by_uuid] {}", msg);
                Err(BusinessError::not_found("Tenant not found".to_string()))
            }
        }
    }

    /// PD-028: uma página de tenants mais o total.
    pub async fn find_page(&self, page: u64, page_size: u64, search: Option<&str>) -> Result<(Vec<Tenant>, u64), BusinessError> {
        let (entities, total) = self.gateway.find_page(page, page_size, search).await.map_err(|e| {
            let msg = format!("Database error: {}", e);
            log::error!("[TenantUseCase::find_page] {}", msg);
            BusinessError::new(msg)
        })?;
        Ok((TenantEntityMapper::from_models(entities), total))
    }

    pub async fn find_all(&self) -> Result<Vec<Tenant>, BusinessError> {
        log::info!("[TenantUseCase::find_all] Executing find_all tenants");

        let entities = self
            .gateway
            .find_all()
            .await
            .map_err(|e| {
                let msg = format!("Database error: {}", e);
                log::error!("[TenantUseCase::find_all] {}", msg);
                BusinessError::new(msg)
            })?;

        Ok(TenantEntityMapper::from_models(entities))
    }

    pub async fn update(&self, id: i64, tenant: Tenant) -> Result<Tenant, BusinessError> {
        log::info!("[TenantUseCase::update] Executing update for id {}: {:?}", id, tenant.business_name);

        let existing = match self.find_by_id(id).await {
            Ok(t) => t,
            Err(e) => {
                log::error!("[TenantUseCase::update] Tenant to update not found with id {}: {}", id, e);
                return Err(e);
            }
        };

        // EPIC-TP-01-S02 (HRMS-201): update validates the same field create
        // does. It used to require `company_name` — which is optional on
        // create — so a tenant onboarded with only its required fields could
        // not be edited at all, not even to add the missing company name
        // (DEF-TP-02), while the business name went unchecked and could be
        // blanked out (DEF-TP-03).
        if !valid_business_name(&tenant.business_name) {
            let msg = "Tenant business name is required".to_string();
            log::error!("[TenantUseCase::update] {}", msg);
            return Err(BusinessError::new(msg));
        }
        if !valid_country_code(&tenant.country_code) {
            return Err(BusinessError::new("Country code must contain two letters".to_string()));
        }
        // valid_country_code above guarantees Some at this point.
        let country_code = tenant.country_code.clone().unwrap();
        let tax_id = validate_tax_id(&country_code, &tenant.tax_id)?;

        let updated_tenant = Tenant {
            id: Some(id),
            uuid: existing.uuid,
            company_name: tenant.company_name,
            business_name:  tenant.business_name,
            tax_id,
            email: tenant.email,
            phone: tenant.phone,
            website: tenant.website,
            address_line1: tenant.address_line1,
            address_line2: tenant.address_line2,
            locality: tenant.locality,
            administrative_area: tenant.administrative_area,
            postal_code: tenant.postal_code,
            country_code: tenant.country_code,
            // A tenant's plan is set only through `set_plan` (HRMS-224,
            // PD-021) — the general update path always keeps it as-is,
            // regardless of what the caller sent.
            business_plan_id: existing.business_plan_id,
            created_at: existing.created_at,
            created_by: existing.created_by,
            updated_at: None,
            updated_by: tenant.updated_by,
        };

        let entity = self
            .gateway
            .persist(updated_tenant)
            .await
            .map_err(|e| {
                let msg = format!("Failed to update tenant: {}", e);
                log::error!("[TenantUseCase::update] {}", msg);
                BusinessError::new(msg)
            })?;

        Ok(TenantEntityMapper::from_active_model(entity))
    }

    pub async fn persist(&self, tenant: Tenant) -> Option<Tenant> {
        log::info!("[TenantUseCase::persist] Executing persist tenant: {:?}", tenant.business_name);
        self.create(tenant).await.ok()
    }

    /// Sets the tenant's single current plan (HRMS-222, PD-021). The caller
    /// (the `/tenant/{id}/plan` endpoint) is responsible for restricting this
    /// to an unbound platform administrator and for validating that
    /// `business_plan_id` refers to an existing plan (HRMS-224).
    pub async fn set_plan(&self, tenant_id: i64, business_plan_id: i64) -> Result<Tenant, BusinessError> {
        log::info!("[TenantUseCase::set_plan] Setting plan {} for tenant {}", business_plan_id, tenant_id);

        let existing = self.find_by_id(tenant_id).await?;
        let updated = Tenant {
            business_plan_id: Some(business_plan_id),
            ..existing
        };

        let entity = self
            .gateway
            .persist(updated)
            .await
            .map_err(|e| {
                let msg = format!("Failed to set tenant plan: {}", e);
                log::error!("[TenantUseCase::set_plan] {}", msg);
                BusinessError::new(msg)
            })?;

        Ok(TenantEntityMapper::from_active_model(entity))
    }
}

#[cfg(test)]
mod tests {
    use super::{valid_business_name, valid_country_code, validate_tax_id};

    #[test]
    fn a_business_name_of_blanks_is_not_a_name() {
        // DEF-TP-03: `is_empty` let "   " through, so a tenant could be
        // created — and on update renamed — to a run of spaces.
        assert!(valid_business_name("Acme"));
        assert!(!valid_business_name(""));
        assert!(!valid_business_name("   "));
        assert!(!valid_business_name("\t\n"));
    }

    #[test]
    fn a_tax_identifier_is_required_in_every_country() {
        // DEF-TP-01: BR refused an empty value only as a side effect of the
        // CPF/CNPJ check. Every country without a validator accepted a tenant
        // with no legal identity.
        for country in ["BR", "US", "CA", "MX", "CL"] {
            assert!(validate_tax_id(country, "").is_err(), "{country} accepted an empty tax id");
            assert!(validate_tax_id(country, "   ").is_err(), "{country} accepted a blank tax id");
        }
    }

    #[test]
    fn country_code_is_now_required_not_optional() {
        // EPIC-TP-01-S04: this is the behaviour change from the prior
        // is_none_or -- None used to be valid, and no longer is.
        assert!(!valid_country_code(&None));
    }

    #[test]
    fn country_code_must_be_exactly_two_ascii_letters() {
        assert!(valid_country_code(&Some("BR".to_string())));
        assert!(valid_country_code(&Some("US".to_string())));
        assert!(!valid_country_code(&Some("BRA".to_string())));
        assert!(!valid_country_code(&Some("B1".to_string())));
        assert!(!valid_country_code(&Some(String::new())));
    }

    #[test]
    fn validate_tax_id_normalizes_a_valid_brazilian_document() {
        assert_eq!(
            validate_tax_id("BR", "529.982.247-25").unwrap(),
            "52998224725"
        );
    }

    #[test]
    fn validate_tax_id_rejects_an_invalid_brazilian_document() {
        assert!(validate_tax_id("BR", "00000000000").is_err());
    }

    #[test]
    fn validate_tax_id_accepts_an_unsupported_countrys_document_as_given() {
        // EPIC-TP-02-S05: no validator for "US" yet -- accepted, not
        // rejected, and not run through Brazil's rules either.
        assert_eq!(validate_tax_id("US", "  12-3456789  ").unwrap(), "12-3456789");
    }
}
