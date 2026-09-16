use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

pub struct BusinessError{
    pub message: String,
}

impl BusinessError {
    pub fn new(message: String) -> Self {
        BusinessError {
            message
        }
    }
}

impl Debug for BusinessError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BusinessError")
            .field("message", &self.message)
            .finish()
    }
}

impl Display for BusinessError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for BusinessError {
    fn description(&self) -> &str {
        &self.message
    }
}
