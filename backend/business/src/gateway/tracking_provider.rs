//! The tracking ingestion boundary (`HRMS-902`, EPIC-FT-01 S03).
//!
//! **Everything specific to the GPS provider lives in this file and nowhere
//! else.** That is the whole requirement: "confine the provider's contract to a
//! single ingestion boundary, so that a change of provider does not propagate
//! beyond it." Above this line the platform speaks
//! `domain::vehicle_tracking`; below it, PinME's JSON.
//!
//! The provider is PinME (`D-15`) — `https://api.pinme.io/api`, a
//! **Traccar-compatible** API, **pull** not push, authenticating with HTTP
//! Basic on every call. There is no `/session`, no token exchange and no
//! bearer token. `D-15` records why the contract did not have to be obtained:
//! `operacao-trm` already integrates this exact provider in production, and
//! `01-project_truth/operacao-trm/01-ai-native/rastreador-status-edge-doc.md`
//! documents the working implementation.
//!
//! **The policy is not here.** When a reading is too old to trust, whether to
//! fall back to the event report, and what to conclude when neither answers,
//! are hermes' rules and live in `use_cases::vehicle_tracking_use_case`. This
//! module only knows how to *ask* and how to translate the reply. Putting the
//! freshness rule here would mean re-deciding it for the next provider.
//!
//! **Credentials never leave this module** — not into an error, not into a log
//! line. The reference implementation makes the same promise and it is worth
//! keeping: an `Authorization` header in a log is a credential in a log.

use crate::domain::business_error::BusinessError;
use base64::Engine;
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::env;

/// A position reading, translated out of the provider's wire format but not
/// yet judged. `valid` and `recorded_at` are carried verbatim so the policy
/// layer can apply the freshness rule; this module does not apply it.
#[derive(Clone, Debug, PartialEq)]
pub struct ProviderPosition {
    pub device_id: i64,
    pub latitude: f64,
    pub longitude: f64,
    /// The provider's own quality flag for the fix.
    pub valid: bool,
    /// `None` when the provider sent no usable timestamp at all.
    pub recorded_at: Option<DateTime<Utc>>,
    /// `None` when the device did not report an ignition attribute — which is
    /// one of the three conditions that sends the policy layer to the events
    /// report.
    pub ignition: Option<bool>,
    pub odometer_meters: Option<f64>,
}

/// An ignition transition from the provider's event report.
#[derive(Clone, Debug, PartialEq)]
pub struct ProviderIgnitionEvent {
    pub device_id: i64,
    /// `true` for `ignitionOn`, `false` for `ignitionOff`.
    pub ignition: bool,
    pub occurred_at: DateTime<Utc>,
}

/// What the platform may ask a GPS provider for.
///
/// Two calls, because the provider offers exactly two useful shapes: the
/// current position of everything, and a historical event window per device.
/// A provider that cannot answer the second is still usable — the policy layer
/// degrades rather than fails (see `VehicleTrackingUseCase`).
pub trait TrackingProvider {
    /// The last known position for **every** device the account can see, in
    /// one call. Traccar's `/positions` is not per-device; asking per device
    /// would multiply the request count by the size of the fleet.
    fn last_positions(
        &self,
    ) -> impl Future<Output = Result<Vec<ProviderPosition>, BusinessError>> + Send;

    /// Ignition transitions for `device_ids` within a window, newest last.
    fn ignition_events(
        &self,
        device_ids: &[i64],
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> impl Future<Output = Result<Vec<ProviderIgnitionEvent>, BusinessError>> + Send;
}

/// The PinME / Traccar adapter.
pub struct PinMeTrackingProvider {
    client: reqwest::Client,
    base_url: String,
    /// Pre-encoded `Basic …`. Held already-encoded so the plaintext password
    /// is not kept around longer than the constructor.
    authorization: String,
}

impl PinMeTrackingProvider {
    /// Reads `TRACKING_API_BASE_URL`, `TRACKING_API_EMAIL` and
    /// `TRACKING_API_PASSWORD`.
    ///
    /// Named for the role, not the vendor: swapping provider should not
    /// require renaming deployment configuration. Configuration is read here
    /// rather than at boot for the same reason `EmailSender` does — a missing
    /// tracking credential blocks one workflow, and is not a reason to refuse
    /// to start the API.
    pub fn from_env() -> Result<Self, BusinessError> {
        let base_url = env::var("TRACKING_API_BASE_URL")
            .map_err(|_| BusinessError::new("TRACKING_API_BASE_URL must be set".to_string()))?;
        let email = env::var("TRACKING_API_EMAIL")
            .map_err(|_| BusinessError::new("TRACKING_API_EMAIL must be set".to_string()))?;
        let password = env::var("TRACKING_API_PASSWORD")
            .map_err(|_| BusinessError::new("TRACKING_API_PASSWORD must be set".to_string()))?;
        Ok(Self::new(&base_url, &email, &password))
    }

    pub fn new(base_url: &str, email: &str, password: &str) -> Self {
        let encoded =
            base64::engine::general_purpose::STANDARD.encode(format!("{email}:{password}"));
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            authorization: format!("Basic {encoded}"),
        }
    }

    async fn get(&self, path: &str, query: &[(String, String)]) -> Result<Value, BusinessError> {
        let url = format!("{}{}", self.base_url, path);
        let response = self
            .client
            .get(&url)
            .header("Authorization", &self.authorization)
            .header("Accept", "application/json")
            .query(query)
            .send()
            .await
            // The error is deliberately not `{e}`-formatted with the URL in
            // it; reqwest renders the request URL, and the credential is in a
            // header rather than the URL, but keeping the habit tight is
            // cheaper than auditing it later.
            .map_err(|_| {
                BusinessError::new(format!("Tracking provider unreachable at {path}"))
            })?;

        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(BusinessError::new(
                "Tracking provider rejected the credentials (HTTP Basic)".to_string(),
            ));
        }
        if !response.status().is_success() {
            return Err(BusinessError::new(format!(
                "Tracking provider returned {} for {path}",
                response.status().as_u16()
            )));
        }
        response
            .json::<Value>()
            .await
            .map_err(|_| BusinessError::new(format!("Tracking provider sent invalid JSON for {path}")))
    }
}

impl TrackingProvider for PinMeTrackingProvider {
    async fn last_positions(&self) -> Result<Vec<ProviderPosition>, BusinessError> {
        Ok(parse_positions(&self.get("/positions", &[]).await?))
    }

    async fn ignition_events(
        &self,
        device_ids: &[i64],
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<ProviderIgnitionEvent>, BusinessError> {
        // Traccar takes repeated `deviceId` and `type` parameters rather than
        // comma-separated lists.
        let mut query: Vec<(String, String)> = device_ids
            .iter()
            .map(|id| ("deviceId".to_string(), id.to_string()))
            .collect();
        query.push(("from".to_string(), from.to_rfc3339()));
        query.push(("to".to_string(), to.to_rfc3339()));
        query.push(("type".to_string(), "ignitionOn".to_string()));
        query.push(("type".to_string(), "ignitionOff".to_string()));
        Ok(parse_ignition_events(&self.get("/reports/events", &query).await?))
    }
}

/// `serverTime` / `fixTime` / `deviceTime`, first present wins — the order
/// `D-15` records from the production integration.
fn first_timestamp(value: &Value) -> Option<DateTime<Utc>> {
    ["serverTime", "fixTime", "deviceTime"]
        .iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .and_then(|raw| DateTime::parse_from_rfc3339(raw).ok())
        .map(|dt| dt.with_timezone(&Utc))
}

/// Accumulated metres. Traccar spells this three ways depending on the device
/// and firmware; `D-15` lists all three as depended upon.
fn odometer(attributes: Option<&Value>) -> Option<f64> {
    let attributes = attributes?;
    ["totalDistance", "odometer", "distance"]
        .iter()
        .find_map(|key| attributes.get(*key).and_then(Value::as_f64))
}

/// Translates `/positions`. A row that carries no `deviceId` is skipped rather
/// than defaulted — a position belonging to no device is not a position.
fn parse_positions(body: &Value) -> Vec<ProviderPosition> {
    body.as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|row| {
            let attributes = row.get("attributes");
            Some(ProviderPosition {
                device_id: row.get("deviceId").and_then(Value::as_i64)?,
                latitude: row.get("latitude").and_then(Value::as_f64).unwrap_or(0.0),
                longitude: row.get("longitude").and_then(Value::as_f64).unwrap_or(0.0),
                // Absent `valid` is treated as valid: Traccar omits it on some
                // firmware, and the freshness rule downstream is the real
                // guard. Treating "absent" as "invalid" would blind the fleet
                // on a field the provider simply did not send.
                valid: row.get("valid").and_then(Value::as_bool).unwrap_or(true),
                recorded_at: first_timestamp(row),
                ignition: attributes.and_then(|a| a.get("ignition")).and_then(Value::as_bool),
                odometer_meters: odometer(attributes),
            })
        })
        .collect()
}

/// Translates `/reports/events`. Anything that is not an ignition transition
/// is dropped — the report is asked for two types but a provider is free to
/// send more.
fn parse_ignition_events(body: &Value) -> Vec<ProviderIgnitionEvent> {
    body.as_array()
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|row| {
            let ignition = match row.get("type").and_then(Value::as_str)? {
                "ignitionOn" => true,
                "ignitionOff" => false,
                _ => return None,
            };
            Some(ProviderIgnitionEvent {
                device_id: row.get("deviceId").and_then(Value::as_i64)?,
                ignition,
                // Events carry `eventTime` on current Traccar; older builds
                // only set `serverTime`. An event with no usable instant is
                // dropped, because "most recent event wins" is meaningless
                // without one.
                occurred_at: row
                    .get("eventTime")
                    .and_then(Value::as_str)
                    .and_then(|raw| DateTime::parse_from_rfc3339(raw).ok())
                    .map(|dt| dt.with_timezone(&Utc))
                    .or_else(|| first_timestamp(row))?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn translates_a_position_row_into_provider_neutral_shape() {
        let body = json!([{
            "deviceId": 393335, "latitude": -23.5, "longitude": -46.6, "valid": true,
            "serverTime": "2026-06-30T18:34:49.402+00:00",
            "attributes": { "ignition": true, "totalDistance": 12345.6 }
        }]);
        let parsed = parse_positions(&body);
        assert_eq!(parsed.len(), 1);
        let p = &parsed[0];
        assert_eq!((p.device_id, p.valid, p.ignition), (393335, true, Some(true)));
        assert_eq!(p.odometer_meters, Some(12345.6));
        assert_eq!(p.recorded_at.unwrap().to_rfc3339(), "2026-06-30T18:34:49.402+00:00");
    }

    #[test]
    fn timestamp_precedence_is_server_then_fix_then_device() {
        let fix = json!({ "fixTime": "2026-01-01T00:00:00+00:00", "deviceTime": "2025-01-01T00:00:00+00:00" });
        assert_eq!(first_timestamp(&fix).unwrap().to_rfc3339(), "2026-01-01T00:00:00+00:00");

        let server = json!({
            "serverTime": "2026-02-02T00:00:00+00:00",
            "fixTime": "2026-01-01T00:00:00+00:00"
        });
        assert_eq!(first_timestamp(&server).unwrap().to_rfc3339(), "2026-02-02T00:00:00+00:00");
        assert!(first_timestamp(&json!({})).is_none());
    }

    #[test]
    fn odometer_accepts_all_three_spellings() {
        assert_eq!(odometer(Some(&json!({ "totalDistance": 10.0 }))), Some(10.0));
        assert_eq!(odometer(Some(&json!({ "odometer": 20.0 }))), Some(20.0));
        assert_eq!(odometer(Some(&json!({ "distance": 30.0 }))), Some(30.0));
        assert_eq!(odometer(Some(&json!({ "speed": 40.0 }))), None);
        assert_eq!(odometer(None), None);
    }

    #[test]
    fn a_missing_valid_flag_is_treated_as_valid() {
        // Traccar omits `valid` on some firmware. Defaulting to false would
        // report a healthy fleet as untrustworthy.
        let parsed = parse_positions(&json!([{ "deviceId": 1, "latitude": 0.0, "longitude": 0.0 }]));
        assert!(parsed[0].valid);
        assert_eq!(parsed[0].ignition, None);
    }

    #[test]
    fn a_row_without_a_device_id_is_skipped_not_defaulted() {
        let parsed = parse_positions(&json!([{ "latitude": 1.0 }, { "deviceId": 7 }]));
        assert_eq!(parsed.iter().map(|p| p.device_id).collect::<Vec<_>>(), vec![7]);
    }

    #[test]
    fn a_non_array_body_yields_no_positions_rather_than_panicking() {
        assert!(parse_positions(&json!({ "error": "nope" })).is_empty());
        assert!(parse_ignition_events(&json!(null)).is_empty());
    }

    #[test]
    fn translates_ignition_events_and_drops_other_types() {
        let body = json!([
            { "deviceId": 1, "type": "ignitionOn", "eventTime": "2026-06-30T10:00:00+00:00" },
            { "deviceId": 1, "type": "ignitionOff", "eventTime": "2026-06-30T11:00:00+00:00" },
            { "deviceId": 1, "type": "geofenceEnter", "eventTime": "2026-06-30T12:00:00+00:00" },
            { "deviceId": 2, "type": "ignitionOn" }
        ]);
        let parsed = parse_ignition_events(&body);
        assert_eq!(parsed.len(), 2, "geofence dropped, and an event with no instant dropped");
        assert!(parsed[0].ignition && !parsed[1].ignition);
    }

    #[test]
    fn an_event_falls_back_to_server_time_when_event_time_is_absent() {
        let body = json!([{ "deviceId": 1, "type": "ignitionOn", "serverTime": "2026-06-30T10:00:00+00:00" }]);
        assert_eq!(parse_ignition_events(&body).len(), 1);
    }

    #[test]
    fn credentials_are_encoded_once_and_the_base_url_is_normalised() {
        let provider = PinMeTrackingProvider::new("https://api.pinme.io/api/", "a@b.c", "secret");
        assert_eq!(provider.base_url, "https://api.pinme.io/api");
        // base64("a@b.c:secret")
        assert_eq!(provider.authorization, "Basic YUBiLmM6c2VjcmV0");
        assert!(!provider.authorization.contains("secret"));
    }

    // ---------------------------------------------------------------- wire
    // The translation above is pure and easy to test. The request itself is
    // not, and it is where `D-15`'s contract actually lives: Basic auth on
    // every call, repeated `deviceId`/`type` parameters, and the error mapping.
    // A one-shot loopback server exercises it without a network or a live
    // provider account.
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Serves exactly one request, then yields the raw request text so the
    /// test can assert on what was actually sent.
    async fn serve_once(
        status: u16,
        reason: &str,
        body: &str,
    ) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bound");
        let addr = listener.local_addr().expect("addressed");
        let (body, reason) = (body.to_string(), reason.to_string());
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accepted");
            let mut buf = vec![0u8; 8192];
            let read = socket.read(&mut buf).await.expect("read");
            let request = String::from_utf8_lossy(&buf[..read]).to_string();
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.expect("wrote");
            socket.flush().await.expect("flushed");
            request
        });
        (format!("http://{addr}"), handle)
    }

    #[tokio::test]
    async fn every_call_carries_http_basic_and_asks_for_json() {
        let body = r#"[{"deviceId":42,"latitude":-23.5,"longitude":-46.6,"valid":true,
                        "serverTime":"2026-06-30T18:34:49.402+00:00",
                        "attributes":{"ignition":true,"totalDistance":99.0}}]"#;
        let (base, server) = serve_once(200, "OK", body).await;
        let positions = PinMeTrackingProvider::new(&base, "a@b.c", "secret")
            .last_positions()
            .await
            .expect("provider answered");

        assert_eq!(positions.len(), 1);
        assert_eq!(positions[0].device_id, 42);

        let request = server.await.expect("server finished");
        assert!(request.starts_with("GET /positions"), "request line was: {request}");
        // D-15: Basic on every call -- no /session, no bearer token.
        assert!(request.contains("authorization: Basic YUBiLmM6c2VjcmV0"), "sent: {request}");
        assert!(!request.to_lowercase().contains("bearer"));
        assert!(request.contains("accept: application/json"), "sent: {request}");
    }

    #[tokio::test]
    async fn the_events_report_repeats_device_and_type_parameters() {
        // Traccar takes repeated parameters rather than comma-separated lists;
        // getting this wrong returns an empty report rather than an error,
        // which would silently look like "no events" forever.
        let (base, server) = serve_once(200, "OK", "[]").await;
        let from = DateTime::parse_from_rfc3339("2026-06-29T18:00:00+00:00").unwrap().with_timezone(&Utc);
        let to = DateTime::parse_from_rfc3339("2026-06-30T18:00:00+00:00").unwrap().with_timezone(&Utc);
        PinMeTrackingProvider::new(&base, "a@b.c", "secret")
            .ignition_events(&[7, 9], from, to)
            .await
            .expect("provider answered");

        let request = server.await.expect("server finished");
        assert!(request.contains("deviceId=7&deviceId=9"), "sent: {request}");
        assert!(request.contains("type=ignitionOn&type=ignitionOff"), "sent: {request}");
        assert!(request.contains("from=2026-06-29T18"), "sent: {request}");
    }

    #[tokio::test]
    async fn rejected_credentials_are_reported_as_such_without_echoing_them() {
        let (base, _server) = serve_once(401, "Unauthorized", "{}").await;
        let error = PinMeTrackingProvider::new(&base, "a@b.c", "hunter2")
            .last_positions()
            .await
            .expect_err("401 is an error");
        let message = format!("{:?}", error);
        assert!(message.contains("credentials"), "unhelpful message: {message}");
        // A credential in an error message is a credential in a log.
        assert!(!message.contains("hunter2"), "the password leaked: {message}");
        assert!(!message.contains("YUBiLmM6"), "the encoded credential leaked: {message}");
    }

    #[tokio::test]
    async fn an_unexpected_status_is_an_error_not_an_empty_fleet() {
        // Returning `[]` on a 500 would read as "every device is unknown",
        // which the policy layer would faithfully report as a dark fleet.
        let (base, _server) = serve_once(500, "Internal Server Error", "nope").await;
        assert!(PinMeTrackingProvider::new(&base, "a@b.c", "x").last_positions().await.is_err());
    }

    #[tokio::test]
    async fn a_non_json_body_is_an_error_not_an_empty_fleet() {
        let (base, _server) = serve_once(200, "OK", "<html>maintenance</html>").await;
        assert!(PinMeTrackingProvider::new(&base, "a@b.c", "x").last_positions().await.is_err());
    }
}
