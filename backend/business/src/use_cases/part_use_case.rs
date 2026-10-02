use crate::domain::business_error::BusinessError;
use crate::domain::part::Part;
use crate::gateway::part_gateway::PartStore;

pub const NAME_REQUIRED: &str = "A part needs a name";
pub const UNIT_REQUIRED: &str = "A part needs a unit";
pub const NEGATIVE_MINIMUM_STOCK: &str = "A part's minimum stock cannot be negative";
pub const PART_NOT_FOUND: &str = "Part not found";

pub struct PartUseCase<G: PartStore> {
    gateway: G,
}

impl<G: PartStore> PartUseCase<G> {
    pub fn new(gateway: G) -> Self {
        Self { gateway }
    }

    pub async fn create(&self, part: Part) -> Result<Part, BusinessError> {
        let part = validated(part)?;
        self.gateway.save(part).await
    }

    pub async fn find_by_uuid(&self, uuid: String) -> Result<Part, BusinessError> {
        match self.gateway.get_by_uuid(uuid).await? {
            Some(part) => Ok(part),
            None => Err(BusinessError::new(PART_NOT_FOUND.to_string())),
        }
    }

    pub async fn find_page(
        &self,
        page: u64,
        page_size: u64,
    ) -> Result<(Vec<Part>, u64), BusinessError> {
        self.gateway.list_page(page, page_size).await
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
