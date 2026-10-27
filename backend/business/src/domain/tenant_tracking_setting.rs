/// `HRMS-900`..`905`: how to reach a tenant's GPS provider.
#[derive(Clone, PartialEq)]
pub struct TrackingCredentials {
    pub base_url: String,
    pub email: String,
    pub password: String,
}

// Hand-written so the password never reaches a log line.
impl std::fmt::Debug for TrackingCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TrackingCredentials")
            .field("base_url", &self.base_url)
            .field("email", &self.email)
            .finish_non_exhaustive()
    }
}
