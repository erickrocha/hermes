use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

pub struct BusinessError{
    pub message: String,
    /// Distinguishes "the record you named does not exist" from every other
    /// business failure, so the HTTP layer can answer 404 instead of 400
    /// (DEF-TP-05). It replaces matching on the message text, which made the
    /// status depend on the wording of a log line.
    not_found: bool,
}

impl BusinessError {
    pub fn new(message: String) -> Self {
        BusinessError {
            message,
            not_found: false,
        }
    }

    pub fn not_found(message: String) -> Self {
        BusinessError {
            message,
            not_found: true,
        }
    }

    pub fn is_not_found(&self) -> bool {
        self.not_found
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
