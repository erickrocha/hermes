use crate::commons::entity_mapper::EntityMapper;
use crate::commons::gateway::Gateway;
use crate::domain::business_error::BusinessError;
use crate::domain::part::{Part, PartEntityMapper};
use crate::gateway::part_gateway::PartGateway;
use sea_orm::DbErr;

pub const NAME_REQUIRED: &str = "A part needs a name";
pub const UNIT_REQUIRED: &str = "A part needs a unit";
pub const NEGATIVE_MINIMUM_STOCK: &str = "A part's minimum stock cannot be negative";
pub const PART_NOT_FOUND: &str = "Part not found";

pub struct PartUseCase {
    gateway: PartGateway,
}

impl PartUseCase {
    pub fn new(gateway: PartGateway) -> Self {
        Self { gateway }
    }

    pub async fn create(&self, part: Part) -> Result<Part, BusinessError> {
        let part = validated(part)?;
        let entity = self.gateway.persist(part).await.map_err(database_error)?;
        Ok(PartEntityMapper::from_active_model(entity))
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<Part, BusinessError> {
        let entity = self.gateway.find_by_uuid(uuid).await.map_err(database_error)?;
        match entity {
            Some(model) => Ok(PartEntityMapper::from_model(model)),
            None => Err(BusinessError::new(PART_NOT_FOUND.to_string())),
        }
    }

    pub async fn find_page(&self, page: u64, page_size: u64) -> Result<(Vec<Part>, u64), BusinessError> {
        let (rows, total) = self.gateway.find_page(page, page_size).await.map_err(database_error)?;
        Ok((PartEntityMapper::from_models(rows), total))
    }
}

fn validated(part: Part) -> Result<Part, BusinessError> {
    let name = part.name.trim().to_string();
    if name.is_empty() {
        return Err(BusinessError::new(NAME_REQUIRED.to_string()));
    }
    let unit = part.unit.trim().to_string();
    if unit.is_empty() {
        return Err(BusinessError::new(UNIT_REQUIRED.to_string()));
    }
    if part.minimum_stock < 0.0 {
        return Err(BusinessError::new(NEGATIVE_MINIMUM_STOCK.to_string()));
    }
    Ok(Part { name, unit, ..part })
}

fn database_error(e: DbErr) -> BusinessError {
    let msg = format!("Database error: {}", e);
    log::error!("[PartUseCase] {}", msg);
    BusinessError::new(msg)
}
