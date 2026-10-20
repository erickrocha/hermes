use crate::commons::entity_mapper::EntityMapper;
use crate::commons::functions::{bytes_para_string, string_to_bytes};
use crate::domain::business_error::BusinessError;
use chrono::NaiveDateTime;
use entity::billing_charge_entity::{ActiveModel as ChargeActiveModel, Model as ChargeModel};
use entity::tenant_billing_entity::{ActiveModel as BillingActiveModel, Model as BillingModel};
use sea_orm::{NotSet, Set, TryIntoModel};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillingChargeStatus {
    Pending,
    Paid,
}

impl BillingChargeStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "Pending",
            Self::Paid => "Paid",
        }
    }
}

impl FromStr for BillingChargeStatus {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "Pending" => Ok(Self::Pending),
            "Paid" => Ok(Self::Paid),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillingAmount {
    pub active_users: i32,
    pub extra_users: i32,
    pub total_in_cents: i64,
}

pub fn calculate_billing_amount(
    base_price_in_cents: i64,
    price_per_extra_user_in_cents: i64,
    included_users: i32,
    active_users: i32,
) -> Result<BillingAmount, BusinessError> {
    if base_price_in_cents < 0 || price_per_extra_user_in_cents < 0 {
        return Err(BusinessError::new("Billing prices cannot be negative".to_string()));
    }
    if included_users <= 0 || active_users < 0 {
        return Err(BusinessError::new("Billing user counts are invalid".to_string()));
    }

    let extra_users = active_users.saturating_sub(included_users).max(0);
    let extra_total = i64::from(extra_users)
        .checked_mul(price_per_extra_user_in_cents)
        .ok_or_else(|| BusinessError::new("Billing amount overflow".to_string()))?;
    let total_in_cents = base_price_in_cents
        .checked_add(extra_total)
        .ok_or_else(|| BusinessError::new("Billing amount overflow".to_string()))?;

    Ok(BillingAmount { active_users, extra_users, total_in_cents })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantBilling {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub grace_days: Option<i32>,
    pub cycle_started_at: Option<NaiveDateTime>,
    pub pending_business_plan_id: Option<i64>,
    pub suspended_at: Option<NaiveDateTime>,
    pub suspended_by: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct TenantBillingEntityMapper;

impl EntityMapper<TenantBilling, BillingModel, BillingActiveModel> for TenantBillingEntityMapper {
    fn build_active_model(value: TenantBilling) -> BillingActiveModel {
        BillingActiveModel {
            id: value.id.map_or(NotSet, Set),
            uuid: value.uuid.map(|uuid| Set(string_to_bytes(&uuid))).unwrap_or(NotSet),
            tenant_id: Set(value.tenant_id),
            grace_days: Set(value.grace_days),
            cycle_started_at: Set(value.cycle_started_at.map(|at| at.and_utc())),
            pending_business_plan_id: Set(value.pending_business_plan_id),
            suspended_at: Set(value.suspended_at.map(|at| at.and_utc())),
            suspended_by: Set(value.suspended_by),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(model: BillingModel) -> TenantBilling {
        TenantBilling {
            id: Some(model.id),
            uuid: Some(bytes_para_string(model.uuid)),
            tenant_id: Some(model.tenant_id.unwrap_or_default()),
            grace_days: model.grace_days,
            cycle_started_at: model.cycle_started_at.map(|at| at.naive_utc()),
            pending_business_plan_id: model.pending_business_plan_id,
            suspended_at: model.suspended_at.map(|at| at.naive_utc()),
            suspended_by: model.suspended_by,
            created_at: Some(model.created_at.naive_utc()),
            created_by: model.created_by,
            updated_at: Some(model.updated_at.naive_utc()),
            updated_by: model.updated_by,
        }
    }

    fn from_active_model(mut model: BillingActiveModel) -> TenantBilling {
        if let Ok(model) = model.clone().try_into_model() {
            return Self::from_model(model);
        }
        TenantBilling {
            id: model.id.take(),
            uuid: model.uuid.take().map(bytes_para_string),
            tenant_id: model.tenant_id.take().flatten(),
            grace_days: model.grace_days.take().flatten(),
            cycle_started_at: model.cycle_started_at.take().flatten().map(|at| at.naive_utc()),
            pending_business_plan_id: model.pending_business_plan_id.take().flatten(),
            suspended_at: model.suspended_at.take().flatten().map(|at| at.naive_utc()),
            suspended_by: model.suspended_by.take().flatten(),
            created_at: model.created_at.take().map(|at| at.naive_utc()),
            created_by: model.created_by.take().flatten(),
            updated_at: model.updated_at.take().map(|at| at.naive_utc()),
            updated_by: model.updated_by.take().flatten(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BillingCharge {
    pub id: Option<i64>,
    pub uuid: Option<String>,
    pub tenant_id: Option<i64>,
    pub tenant_uuid: String,
    pub period_start: NaiveDateTime,
    pub period_end: NaiveDateTime,
    pub tenant_name: String,
    pub plan_name: String,
    pub currency: String,
    pub price_in_cents: i64,
    pub price_per_extra_user_in_cents: i64,
    pub included_users: i32,
    pub active_users: i32,
    pub extra_users: i32,
    pub grace_days: i32,
    pub total_in_cents: i64,
    pub status: BillingChargeStatus,
    pub paid_at: Option<NaiveDateTime>,
    pub paid_by: Option<String>,
    pub created_at: Option<NaiveDateTime>,
    pub created_by: Option<String>,
    pub updated_at: Option<NaiveDateTime>,
    pub updated_by: Option<String>,
}

pub struct BillingChargeEntityMapper;

impl EntityMapper<BillingCharge, ChargeModel, ChargeActiveModel> for BillingChargeEntityMapper {
    fn build_active_model(value: BillingCharge) -> ChargeActiveModel {
        ChargeActiveModel {
            id: value.id.map_or(NotSet, Set),
            uuid: value.uuid.map(|uuid| Set(string_to_bytes(&uuid))).unwrap_or(NotSet),
            tenant_id: Set(value.tenant_id),
            tenant_uuid: Set(string_to_bytes(&value.tenant_uuid)),
            period_start: Set(value.period_start.and_utc()),
            period_end: Set(value.period_end.and_utc()),
            tenant_name: Set(value.tenant_name),
            plan_name: Set(value.plan_name),
            currency: Set(value.currency),
            price_in_cents: Set(value.price_in_cents),
            price_per_extra_user_in_cents: Set(value.price_per_extra_user_in_cents),
            included_users: Set(value.included_users),
            active_users: Set(value.active_users),
            extra_users: Set(value.extra_users),
            grace_days: Set(value.grace_days),
            total_in_cents: Set(value.total_in_cents),
            status: Set(value.status.as_str().to_string()),
            paid_at: Set(value.paid_at.map(|at| at.and_utc())),
            paid_by: Set(value.paid_by),
            created_at: NotSet,
            created_by: NotSet,
            updated_at: NotSet,
            updated_by: NotSet,
        }
    }

    fn from_model(model: ChargeModel) -> BillingCharge {
        BillingCharge {
            id: Some(model.id),
            uuid: Some(bytes_para_string(model.uuid)),
            tenant_id: Some(model.tenant_id.unwrap_or_default()),
            tenant_uuid: bytes_para_string(model.tenant_uuid),
            period_start: model.period_start.naive_utc(),
            period_end: model.period_end.naive_utc(),
            tenant_name: model.tenant_name,
            plan_name: model.plan_name,
            currency: model.currency,
            price_in_cents: model.price_in_cents,
            price_per_extra_user_in_cents: model.price_per_extra_user_in_cents,
            included_users: model.included_users,
            active_users: model.active_users,
            extra_users: model.extra_users,
            grace_days: model.grace_days,
            total_in_cents: model.total_in_cents,
            status: model.status.parse().unwrap_or(BillingChargeStatus::Pending),
            paid_at: model.paid_at.map(|at| at.naive_utc()),
            paid_by: model.paid_by,
            created_at: Some(model.created_at.naive_utc()),
            created_by: model.created_by,
            updated_at: Some(model.updated_at.naive_utc()),
            updated_by: model.updated_by,
        }
    }

    fn from_active_model(mut model: ChargeActiveModel) -> BillingCharge {
        if let Ok(model) = model.clone().try_into_model() {
            return Self::from_model(model);
        }
        BillingCharge {
            id: model.id.take(),
            uuid: model.uuid.take().map(bytes_para_string),
            tenant_id: model.tenant_id.take().flatten(),
            tenant_uuid: model.tenant_uuid.take().map(bytes_para_string).unwrap_or_default(),
            period_start: model.period_start.take().unwrap_or_default().naive_utc(),
            period_end: model.period_end.take().unwrap_or_default().naive_utc(),
            tenant_name: model.tenant_name.take().unwrap_or_default(),
            plan_name: model.plan_name.take().unwrap_or_default(),
            currency: model.currency.take().unwrap_or_else(|| "BRL".to_string()),
            price_in_cents: model.price_in_cents.take().unwrap_or_default(),
            price_per_extra_user_in_cents: model.price_per_extra_user_in_cents.take().unwrap_or_default(),
            included_users: model.included_users.take().unwrap_or_default(),
            active_users: model.active_users.take().unwrap_or_default(),
            extra_users: model.extra_users.take().unwrap_or_default(),
            grace_days: model.grace_days.take().unwrap_or_default(),
            total_in_cents: model.total_in_cents.take().unwrap_or_default(),
            status: model.status.take().and_then(|s| s.parse().ok()).unwrap_or(BillingChargeStatus::Pending),
            paid_at: model.paid_at.take().flatten().map(|at| at.naive_utc()),
            paid_by: model.paid_by.take().flatten(),
            created_at: model.created_at.take().map(|at| at.naive_utc()),
            created_by: model.created_by.take().flatten(),
            updated_at: model.updated_at.take().map(|at| at.naive_utc()),
            updated_by: model.updated_by.take().flatten(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::calculate_billing_amount;

    #[test]
    fn charges_only_enabled_human_users_above_the_included_count() {
        let below = calculate_billing_amount(10_000, 500, 10, 8).unwrap();
        assert_eq!((below.extra_users, below.total_in_cents), (0, 10_000));

        let at_limit = calculate_billing_amount(10_000, 500, 10, 10).unwrap();
        assert_eq!((at_limit.extra_users, at_limit.total_in_cents), (0, 10_000));

        let above = calculate_billing_amount(10_000, 500, 10, 13).unwrap();
        assert_eq!((above.extra_users, above.total_in_cents), (3, 11_500));
    }

    #[test]
    fn zero_price_and_zero_overage_are_valid_but_negative_prices_are_not() {
        assert_eq!(calculate_billing_amount(0, 0, 5, 7).unwrap().total_in_cents, 0);
        assert!(calculate_billing_amount(100, -1, 5, 7).is_err());
    }

    #[test]
    fn rejects_an_amount_that_overflows_integer_cents() {
        assert!(calculate_billing_amount(i64::MAX, 1, 0, i32::MAX).is_err());
    }
}