use sea_orm::entity::prelude::*;

/// `HRMS-900`..`905`: the tenant's GPS provider account. The password is
/// stored as given because the provider's HTTP Basic call needs it back; it is
/// never read out by the API.
#[derive(Clone, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "tenant_tracking_setting")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    #[sea_orm(unique)]
    pub uuid: Vec<u8>,
    pub tenant_id: Option<i64>,
    pub base_url: String,
    pub email: String,
    pub password: String,
    pub created_at: DateTimeUtc,
    pub created_by: Option<String>,
    pub updated_at: DateTimeUtc,
    pub updated_by: Option<String>,
}

// Hand-written so a logged row never carries the password.
impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Model")
            .field("tenant_id", &self.tenant_id)
            .field("base_url", &self.base_url)
            .field("email", &self.email)
            .finish_non_exhaustive()
    }
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

crate::impl_tenant_auditable_before_save!(ActiveModel);
