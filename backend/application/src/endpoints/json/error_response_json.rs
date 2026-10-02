use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponseJson {
    pub error_key: String,
    pub message: String,
}

impl ErrorResponseJson {
    pub fn new(error_key: String, message: String) -> Self {
        Self { error_key, message }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BadRequestErrorJson {
    #[schema(example = "BadRequest")]
    pub error: String,
    #[schema(example = "Invalid request payload")]
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UnauthorizedErrorJson {
    #[schema(example = "Unauthorized")]
    pub error: String,
    #[schema(example = "Token is invalid")]
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ForbiddenErrorJson {
    #[schema(example = "Forbidden")]
    pub error: String,
    #[schema(example = "Please add the JWT token to the header")]
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct InternalServerErrorJson {
    #[schema(example = "InternalServerError")]
    pub error: String,
    #[schema(example = "Unexpected server error")]
    pub message: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotFoundErrorJson {
    #[schema(example = "NotFound")]
    pub error: String,
    #[schema(example = "Resource not found")]
    pub message: String,
}
