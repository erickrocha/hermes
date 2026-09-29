//! The fuel-acquisition ingestion boundary (`HRMS-944`, `EPIC-FU-02-S01`).
//!
//! **Everything specific to the fuel-management provider lives in this file
//! and nowhere else** -- the same split `gateway::tracking_provider` already
//! established for PinME (`HRMS-902`).
//!
//! The provider is **CTA Smart** (`D-25`) -- `https://www.ctasmart.com.br:8443`
//! by default, bearer token, a **pull queue**: fetch the oldest unprocessed
//! transactions, then acknowledge each one to advance the queue. `D-25`
//! records why the contract did not have to be obtained: `operacao-trm`
//! already integrates this exact provider in production, and
//! `04-codebase/operacao-trm/supabase/functions/abastecimentos-sync/index.ts`
//! (304 lines) is a complete, working reference implementation -- every field
//! name, query parameter and status code below is taken from it directly, not
//! guessed at.
//!
//! **The policy is not here.** Which vehicle a transaction matches, whether it
//! is a correction or a new fuelling, what gets written to the ledger --
//! those are hermes' own rules and live in `use_cases::fuel_sync_use_case`.
//! This module only knows how to *ask* and how to translate the reply.
//!
//! **Credentials never leave this module** -- the same promise
//! `tracking_provider` makes and the reference implementation itself makes
//! (`Authorization` header, never logged).

use crate::domain::business_error::BusinessError;
use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use serde_json::{Value, json};
use std::env;

/// A fuelling transaction, translated out of the provider's wire format but
/// not yet judged -- vehicle matching and the correction-vs-new decision are
/// `FuelSyncUseCase`'s job, not this module's.
#[derive(Clone, Debug, PartialEq)]
pub struct ProviderFuelTransaction {
    /// The provider's own transaction id (`TRM-511`) -- never empty; a row
    /// with none is dropped during translation, the same "nothing to key on"
    /// treatment the reference implementation gives it (`if (!extId)
    /// continue`, no acknowledgement for that row at all).
    pub external_id: String,
    /// `veiculo.frota` -- matched against a vehicle's `prefix` first
    /// (`TRM-507`).
    pub fleet_prefix: Option<String>,
    /// `veiculo.placa`, not yet normalised -- the caller normalises the same
    /// way `VehicleUseCase::normalise_plate` does.
    pub plate: Option<String>,
    /// `veiculo.nome` -- carried through only so a future ignore-list
    /// (`TRM-509`) has something to test against; nothing reads it yet.
    pub fleet_name: Option<String>,
    /// `None` when `data_inicio`/`hora_inicio` could not be parsed -- the
    /// reference implementation always falls back to "now" (`toISO`'s own
    /// catch-all), which this module deliberately does not reproduce: a
    /// fuelling dated "now" because the real date failed to parse is a wrong
    /// fact recorded confidently, not a degraded one.
    pub recorded_at: Option<DateTime<Utc>>,
    pub volume_liters: f64,
    pub value_cents: i64,
    pub odometer_km: Option<f64>,
    pub station: Option<String>,
    pub full_tank: bool,
}

/// What the sync tells the provider about one transaction, advancing the
/// queue (`TRM-510`: only after the write succeeded).
#[derive(Clone, Debug, PartialEq)]
pub enum AckStatus {
    /// Imported, updated, or deliberately refused -- all three leave the
    /// queue (`TRM-509`).
    Success,
    /// Recoverable -- stays effectively in the queue via the provider's own
    /// REVERTIDO mechanism (`TRM-508`).
    Pending(String),
}

/// What the platform may ask the fuel-management provider for.
pub trait FuelProvider {
    /// The oldest unprocessed batch, oldest first (`TRM-501`), capped at 100
    /// records (`TRM-502`) -- the fixed value the reference implementation
    /// itself hardcodes, not read from configuration.
    fn fetch_pending(&self) -> impl Future<Output = Result<Vec<ProviderFuelTransaction>, BusinessError>> + Send;

    /// Advances the queue for exactly the records named, each carrying its
    /// own outcome (`TRM-501`, per-record `TRM-508`/`510`).
    fn acknowledge(
        &self,
        results: &[(String, AckStatus)],
    ) -> impl Future<Output = Result<(), BusinessError>> + Send;
}

/// The provider this deployment is configured for -- named for the role
/// everywhere else (`HRMS-902`'s own discipline), never for the vendor.
pub fn configured_provider() -> Result<impl FuelProvider, BusinessError> {
    CtaSmartFuelProvider::from_env()
}

/// The CTA Smart adapter.
pub struct CtaSmartFuelProvider {
    client: reqwest::Client,
    base_url: String,
    authorization: String,
}

impl CtaSmartFuelProvider {
    /// Reads `FUEL_API_BASE_URL` (defaults to CTA Smart's own address, the
    /// same default the reference implementation falls back to) and
    /// `FUEL_API_TOKEN`.
    pub fn from_env() -> Result<Self, BusinessError> {
        let base_url = env::var("FUEL_API_BASE_URL")
            .unwrap_or_else(|_| "https://www.ctasmart.com.br:8443".to_string());
        let token = env::var("FUEL_API_TOKEN")
            .map_err(|_| BusinessError::new("FUEL_API_TOKEN must be set".to_string()))?;
        Ok(Self::new(&base_url, &token))
    }

    pub fn new(base_url: &str, token: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            authorization: format!("Bearer {token}"),
        }
    }
}

impl FuelProvider for CtaSmartFuelProvider {
    async fn fetch_pending(&self) -> Result<Vec<ProviderFuelTransaction>, BusinessError> {
        let url = format!(
            "{}/SvWebSincronizaAbastecimentos?formato=json&max_linhas=100&tipo_produto=0",
            self.base_url
        );
        let response = self
            .client
            .get(&url)
            .header("Authorization", &self.authorization)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|_| BusinessError::new("Fuel provider unreachable".to_string()))?;

        if !response.status().is_success() {
            return Err(BusinessError::new(format!(
                "Fuel provider returned {} for SvWebSincronizaAbastecimentos",
                response.status().as_u16()
            )));
        }
        let body = response.json::<Value>().await.map_err(|_| {
            BusinessError::new("Fuel provider sent invalid JSON for SvWebSincronizaAbastecimentos".to_string())
        })?;
        Ok(parse_transactions(&body))
    }

    async fn acknowledge(&self, results: &[(String, AckStatus)]) -> Result<(), BusinessError> {
        let acks: Vec<Value> = results
            .iter()
            .map(|(id, status)| match status {
                AckStatus::Success => json!({ "id": id, "status": "SUCESSO" }),
                AckStatus::Pending(reason) => json!({ "id": id, "status": "PENDENTE", "motivo_erro": reason }),
            })
            .collect();
        let url = format!("{}/SvWebInformaSincronismoAbastecimentos?formato=json", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Authorization", &self.authorization)
            .header("Content-Type", "application/json")
            .json(&json!({ "abastecimentos": acks }))
            .send()
            .await
            .map_err(|_| BusinessError::new("Fuel provider unreachable acknowledging the batch".to_string()))?;

        if !response.status().is_success() {
            return Err(BusinessError::new(format!(
                "Fuel provider returned {} for SvWebInformaSincronismoAbastecimentos",
                response.status().as_u16()
            )));
        }
        Ok(())
    }
}

/// `TRM-598`: the provider's datetime carries no offset and is local
/// Brazil time. Brazil has observed no daylight saving since 2019, so a
/// fixed `-03:00` offset is exact, not an approximation -- the same `TC`
/// value `TRM-598` itself cites.
fn parse_brt_datetime(data: &str, hora: &str) -> Option<DateTime<Utc>> {
    let mut parts = data.trim().split('/');
    let day: u32 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let year: i32 = parts.next()?.parse().ok()?;
    let time = hora.trim().get(0..5).unwrap_or("00:00");
    let mut time_parts = time.split(':');
    let hour: u32 = time_parts.next()?.parse().ok()?;
    let minute: u32 = time_parts.next()?.parse().ok()?;

    let naive = chrono::NaiveDate::from_ymd_opt(year, month, day)?.and_hms_opt(hour, minute, 0)?;
    let brt = FixedOffset::west_opt(3 * 3600)?;
    let local = brt.from_local_datetime(&naive).single()?;
    Some(local.with_timezone(&Utc))
}

/// `parseBr` in the reference implementation: `"1.365,56"` -> `1365.56`.
/// Brazilian numbers use `.` as the thousands separator and `,` as the
/// decimal point -- the opposite of what `str::parse` expects.
fn parse_br_number(raw: &str) -> f64 {
    let cleaned = raw.trim().replace('.', "").replace(',', ".");
    cleaned.parse().unwrap_or(0.0)
}

/// Digits only, the same `String(v).replace(/[^\d]/g, "")` the reference
/// implementation applies to the odometer before parsing it as an integer.
fn digits_only(raw: &str) -> Option<f64> {
    let digits: String = raw.chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() { None } else { digits.parse().ok() }
}

/// Translates `{"abastecimentos": [...]}`. A row with no `id` is dropped
/// entirely, the same "nothing to key on" treatment the reference
/// implementation gives it.
fn parse_transactions(body: &Value) -> Vec<ProviderFuelTransaction> {
    body.get("abastecimentos")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(|row| {
            let external_id = row.get("id").and_then(Value::as_str)?.trim().to_string();
            if external_id.is_empty() {
                return None;
            }
            let vehicle = row.get("veiculo");
            let data = row.get("data_inicio").and_then(Value::as_str).unwrap_or_default();
            let hora = row.get("hora_inicio").and_then(Value::as_str).unwrap_or_default();
            // `volume_fixed` wins over `volume` -- the reference
            // implementation's own precedence.
            let volume_raw = row
                .get("volume_fixed")
                .or_else(|| row.get("volume"))
                .and_then(Value::as_str)
                .unwrap_or_default();
            let full_tank = row
                .get("completo")
                .and_then(Value::as_str)
                .map(|s| s == "true")
                .unwrap_or(false);

            Some(ProviderFuelTransaction {
                external_id,
                fleet_prefix: vehicle
                    .and_then(|v| v.get("frota"))
                    .and_then(Value::as_str)
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
                plate: vehicle
                    .and_then(|v| v.get("placa"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .filter(|s| !s.is_empty()),
                fleet_name: vehicle
                    .and_then(|v| v.get("nome"))
                    .and_then(Value::as_str)
                    .map(str::to_string),
                recorded_at: parse_brt_datetime(data, hora),
                volume_liters: parse_br_number(volume_raw),
                value_cents: (parse_br_number(
                    row.get("custo").and_then(Value::as_str).unwrap_or_default(),
                ) * 100.0)
                    .round() as i64,
                odometer_km: row
                    .get("odometro")
                    .and_then(Value::as_str)
                    .and_then(digits_only),
                station: row
                    .get("posto")
                    .and_then(|p| p.get("nome"))
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .filter(|s| !s.is_empty()),
                full_tank,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn translates_a_transaction_row_into_provider_neutral_shape() {
        let body = json!({ "abastecimentos": [{
            "id": "ext-1",
            "veiculo": { "frota": "2642", "placa": "ABC1D23", "nome": "Ônibus 2642", "id": "v1" },
            "data_inicio": "30/09/2026", "hora_inicio": "14:30:00",
            "volume_fixed": "98,629", "custo": "740,70",
            "odometro": "49.528 km", "posto": { "nome": "Posto Central" },
            "completo": "true"
        }]});
        let parsed = parse_transactions(&body);
        assert_eq!(parsed.len(), 1);
        let tx = &parsed[0];
        assert_eq!(tx.external_id, "ext-1");
        assert_eq!(tx.fleet_prefix.as_deref(), Some("2642"));
        assert_eq!(tx.plate.as_deref(), Some("ABC1D23"));
        assert_eq!(tx.volume_liters, 98.629);
        assert_eq!(tx.value_cents, 74_070);
        assert_eq!(tx.odometer_km, Some(49_528.0));
        assert_eq!(tx.station.as_deref(), Some("Posto Central"));
        assert!(tx.full_tank);
        assert_eq!(tx.recorded_at.unwrap().to_rfc3339(), "2026-09-30T17:30:00+00:00");
    }

    #[test]
    fn a_row_with_no_id_is_dropped_not_defaulted() {
        let body = json!({ "abastecimentos": [{ "veiculo": {} }, { "id": "ext-2" }] });
        let parsed = parse_transactions(&body);
        assert_eq!(parsed.iter().map(|t| t.external_id.as_str()).collect::<Vec<_>>(), vec!["ext-2"]);
    }

    #[test]
    fn volume_prefers_the_fixed_field_over_the_raw_one() {
        let body = json!({ "abastecimentos": [{
            "id": "ext-3", "volume_fixed": "10,0", "volume": "999,0", "custo": "0"
        }]});
        assert_eq!(parse_transactions(&body)[0].volume_liters, 10.0);
    }

    #[test]
    fn an_unparsable_datetime_is_none_not_now() {
        let body = json!({ "abastecimentos": [{ "id": "ext-4", "data_inicio": "", "custo": "0" }]});
        assert_eq!(parse_transactions(&body)[0].recorded_at, None);
    }

    #[test]
    fn a_non_object_body_yields_no_transactions_rather_than_panicking() {
        assert!(parse_transactions(&json!({ "error": "nope" })).is_empty());
        assert!(parse_transactions(&json!(null)).is_empty());
    }

    #[test]
    fn brazilian_number_format_is_parsed_correctly() {
        assert_eq!(parse_br_number("1.365,56"), 1365.56);
        assert_eq!(parse_br_number("52,01"), 52.01);
        assert_eq!(parse_br_number("55210"), 55210.0);
        assert_eq!(parse_br_number(""), 0.0);
    }

    #[test]
    fn brt_datetime_converts_to_utc_with_a_fixed_three_hour_offset() {
        // Brazil has observed no DST since 2019 (TRM-598's own TC).
        let dt = parse_brt_datetime("01/01/2026", "00:00").unwrap();
        assert_eq!(dt.to_rfc3339(), "2026-01-01T03:00:00+00:00");
    }

    // ---------------------------------------------------------------- wire
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn serve_once(status: u16, reason: &str, body: &str) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bound");
        let addr = listener.local_addr().expect("addressed");
        let (body, reason) = (body.to_string(), reason.to_string());
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.expect("accepted");
            let mut buf = vec![0u8; 8192];
            let read = socket.read(&mut buf).await.expect("read");
            let request = String::from_utf8_lossy(&buf[..read]).to_string();
            let response = format!(
                "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.expect("wrote");
            socket.flush().await.expect("flushed");
            request
        });
        (format!("http://{addr}"), handle)
    }

    #[tokio::test]
    async fn fetching_carries_a_bearer_token_not_basic_auth() {
        let (base, server) = serve_once(200, "OK", r#"{"abastecimentos":[]}"#).await;
        CtaSmartFuelProvider::new(&base, "secret-token")
            .fetch_pending()
            .await
            .expect("provider answered");
        let request = server.await.expect("server finished");
        assert!(
            request.starts_with("GET /SvWebSincronizaAbastecimentos"),
            "request line was: {request}"
        );
        assert!(request.contains("max_linhas=100"), "sent: {request}");
        assert!(request.contains("authorization: Bearer secret-token"), "sent: {request}");
        assert!(!request.to_lowercase().contains("basic"));
    }

    #[tokio::test]
    async fn acknowledging_posts_the_status_per_record() {
        let (base, server) = serve_once(200, "OK", "{}").await;
        CtaSmartFuelProvider::new(&base, "secret-token")
            .acknowledge(&[
                ("ext-1".to_string(), AckStatus::Success),
                ("ext-2".to_string(), AckStatus::Pending("Vehicle not found".to_string())),
            ])
            .await
            .expect("provider answered");
        let request = server.await.expect("server finished");
        assert!(request.starts_with("POST /SvWebInformaSincronismoAbastecimentos"), "sent: {request}");
        assert!(request.contains(r#""status":"SUCESSO""#), "sent: {request}");
        assert!(request.contains(r#""status":"PENDENTE""#), "sent: {request}");
        assert!(request.contains("Vehicle not found"), "sent: {request}");
    }

    #[tokio::test]
    async fn an_unexpected_status_is_an_error_not_an_empty_batch() {
        let (base, _server) = serve_once(500, "Internal Server Error", "nope").await;
        assert!(CtaSmartFuelProvider::new(&base, "t").fetch_pending().await.is_err());
    }
}
