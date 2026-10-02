use serde::Serialize;
use utoipa::ToSchema;

/// PD-027: o que a importação fez. `created` + `updated` é sempre o total de
/// linhas enviadas, porque o lote é tudo ou nada -- não existe "ignoradas".
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportResultJson {
    pub created: u64,
    pub updated: u64,
}
