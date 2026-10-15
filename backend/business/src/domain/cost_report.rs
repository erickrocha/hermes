//! `C-030` `EPIC-SP-07`/`SP-08` (`HRMS-804…810`): what a vehicle cost over a period, its result, and the two cost
//! reports. Pure: the gateway reads the rows, this composes, groups and writes the CSV.
//!
//! Dates are UTC calendar days. shortcut: the tenant's own day boundary (`garageUtcOffsetMinutes`) is not applied
//! here, so a fuelling at 23:30 local can land in the next day's period; apply the offset when a tenant reports it.

use chrono::{NaiveDate, NaiveDateTime};
use std::collections::BTreeMap;

pub const TO_BE_CLASSIFIED: &str = "";

/// A work order's day in a report: its conclusion, falling back to its opening (`TRM-684`).
pub fn work_order_date(concluded: Option<NaiveDate>, opened: NaiveDateTime) -> NaiveDate {
    concluded.unwrap_or_else(|| opened.date())
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkOrderLine {
    pub uuid: String,
    pub date: NaiveDate,
    pub service_type: Option<String>,
    pub description: String,
    pub postings_cents: i64,
    pub external_cents: i64,
}

impl WorkOrderLine {
    /// `HRMS-808`: postings plus the external-service invoice value; nothing else is recorded on a work order.
    pub fn total_cents(&self) -> i64 {
        self.postings_cents + self.external_cents
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DatedLine {
    pub date: NaiveDate,
    pub reference: String,
    pub description: String,
    pub value_cents: i64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct VehicleCostInput {
    pub work_orders: Vec<WorkOrderLine>,
    pub fuel: Vec<DatedLine>,
    pub expenses: Vec<DatedLine>,
    /// Freight of the vehicle's non-cancelled trips in the period, and how many such trips carried a value.
    pub freight_cents: i64,
    pub freight_trips: i64,
    /// Services performed (stamped), per service name.
    pub services: Vec<(String, i64)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CostBySource {
    pub work_order_postings_cents: i64,
    pub external_services_cents: i64,
    pub fuel_cents: i64,
    pub expenses_cents: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VehicleCost {
    pub by_source: CostBySource,
    pub total_cost_cents: i64,
    /// `HRMS-807`: work-order cost per service type; `""` is "to be classified".
    pub by_service_type: Vec<(String, i64)>,
    pub services_performed: Vec<(String, i64)>,
    pub revenue_cents: i64,
    /// `HRMS-806`: revenue less cost, only when the vehicle earned freight in the period; a vehicle on recurring
    /// service has no priced trips and shows no result rather than a misleading loss.
    pub result_cents: Option<i64>,
}

pub fn compose(input: &VehicleCostInput) -> VehicleCost {
    let by_source = CostBySource {
        work_order_postings_cents: input.work_orders.iter().map(|w| w.postings_cents).sum(),
        external_services_cents: input.work_orders.iter().map(|w| w.external_cents).sum(),
        fuel_cents: input.fuel.iter().map(|l| l.value_cents).sum(),
        expenses_cents: input.expenses.iter().map(|l| l.value_cents).sum(),
    };
    let total = by_source.work_order_postings_cents + by_source.external_services_cents + by_source.fuel_cents + by_source.expenses_cents;
    let mut groups: BTreeMap<String, i64> = BTreeMap::new();
    for order in &input.work_orders {
        let key = order.service_type.as_deref().map(str::trim).filter(|s| !s.is_empty()).unwrap_or(TO_BE_CLASSIFIED);
        *groups.entry(key.to_string()).or_default() += order.total_cents();
    }
    VehicleCost {
        by_source,
        total_cost_cents: total,
        by_service_type: groups.into_iter().collect(),
        services_performed: input.services.clone(),
        revenue_cents: input.freight_cents,
        result_cents: (input.freight_trips > 0).then_some(input.freight_cents - total),
    }
}

fn field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) || value.starts_with(['=', '+', '-', '@']) {
        // The leading apostrophe stops a spreadsheet from running a typed value as a formula.
        let safe = if value.starts_with(['=', '+', '-', '@']) { format!("'{value}") } else { value.to_string() };
        format!("\"{}\"", safe.replace('"', "\"\""))
    } else {
        value.to_string()
    }
}

fn row(cells: &[&str]) -> String {
    cells.iter().map(|c| field(c)).collect::<Vec<_>>().join(",") + "\r\n"
}

pub fn money(cents: i64) -> String {
    let sign = if cents < 0 { "-" } else { "" };
    format!("{sign}{}.{:02}", cents.abs() / 100, cents.abs() % 100)
}

/// `HRMS-809`: one block per cost source, each with its own header and subtotal. UTF-8 with a BOM so Excel
/// reads the accents.
pub fn vehicle_csv(plate: &str, from: NaiveDate, to: NaiveDate, input: &VehicleCostInput) -> String {
    let mut out = String::from("\u{feff}");
    out += &row(&["Vehicle", plate, "From", &from.to_string(), "To", &to.to_string()]);
    let mut block = |title: &str, lines: Vec<(String, String, String, i64)>| {
        out += "\r\n";
        out += &row(&[title]);
        out += &row(&["Date", "Reference", "Description", "Value"]);
        for (date, reference, description, cents) in &lines {
            out += &row(&[date, reference, description, &money(*cents)]);
        }
        out += &row(&["Subtotal", "", "", &money(lines.iter().map(|l| l.3).sum())]);
    };
    block("Work-order postings", input.work_orders.iter().map(|w| (w.date.to_string(), w.uuid.clone(), w.description.clone(), w.postings_cents)).collect());
    block("External services", input.work_orders.iter().filter(|w| w.external_cents != 0).map(|w| (w.date.to_string(), w.uuid.clone(), w.description.clone(), w.external_cents)).collect());
    block("Fuel", input.fuel.iter().map(|l| (l.date.to_string(), l.reference.clone(), l.description.clone(), l.value_cents)).collect());
    block("Expenses", input.expenses.iter().map(|l| (l.date.to_string(), l.reference.clone(), l.description.clone(), l.value_cents)).collect());
    out
}

#[derive(Debug, Clone, PartialEq)]
pub struct SupplierLine {
    pub date: NaiveDate,
    pub invoice_number: Option<String>,
    pub service: String,
    pub work_order_type: Option<String>,
    pub plate: String,
    pub invoice_cents: i64,
}

/// `HRMS-810`: only the supplier's external-service work orders, nothing of freight, fuel or tolls.
pub fn supplier_csv(supplier: &str, from: NaiveDate, to: NaiveDate, lines: &[SupplierLine]) -> String {
    let mut out = String::from("\u{feff}");
    out += &row(&["Supplier", supplier, "From", &from.to_string(), "To", &to.to_string()]);
    out += &row(&["Date", "Invoice", "Service", "Type", "Vehicle", "Value"]);
    for l in lines {
        out += &row(&[&l.date.to_string(), l.invoice_number.as_deref().unwrap_or(""), &l.service, l.work_order_type.as_deref().unwrap_or(""), &l.plate, &money(l.invoice_cents)]);
    }
    out += &row(&["Total", "", "", "", "", &money(lines.iter().map(|l| l.invoice_cents).sum())]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
    }

    fn order(service_type: Option<&str>, postings: i64, external: i64) -> WorkOrderLine {
        WorkOrderLine { uuid: "wo".into(), date: d(5), service_type: service_type.map(String::from), description: "Brakes".into(), postings_cents: postings, external_cents: external }
    }

    #[test]
    fn a_work_order_is_dated_by_its_conclusion_else_its_opening() {
        let opened = d(2).and_hms_opt(23, 0, 0).unwrap();
        assert_eq!(work_order_date(Some(d(9)), opened), d(9));
        assert_eq!(work_order_date(None, opened), d(2));
    }

    #[test]
    fn cost_is_the_sum_of_four_sources_and_groups_keep_unclassified_visible() {
        let input = VehicleCostInput {
            work_orders: vec![order(Some("Brakes"), 1000, 500), order(None, 200, 0), order(Some("  "), 50, 0)],
            fuel: vec![DatedLine { date: d(3), reference: "f".into(), description: "".into(), value_cents: 3000 }],
            expenses: vec![DatedLine { date: d(4), reference: "e".into(), description: "toll".into(), value_cents: 700 }],
            ..Default::default()
        };
        let cost = compose(&input);
        assert_eq!(cost.total_cost_cents, 1000 + 500 + 200 + 50 + 3000 + 700);
        assert_eq!(cost.by_service_type, vec![("".to_string(), 250), ("Brakes".to_string(), 1500)]);
        assert_eq!(cost.result_cents, None, "no priced trip, no result");
    }

    #[test]
    fn the_result_is_revenue_less_cost_only_when_there_is_revenue() {
        let mut input = VehicleCostInput { work_orders: vec![order(None, 1000, 0)], freight_cents: 5000, freight_trips: 1, ..Default::default() };
        assert_eq!(compose(&input).result_cents, Some(4000));
        input.freight_cents = 0;
        input.freight_trips = 0;
        assert_eq!(compose(&input).result_cents, None);
    }

    #[test]
    fn csv_quotes_commas_and_defuses_formulas() {
        let input = VehicleCostInput {
            work_orders: vec![WorkOrderLine { description: "=SUM(A1)".into(), ..order(None, 100, 0) }, WorkOrderLine { description: "a, \"b\"".into(), ..order(None, 250, 0) }],
            ..Default::default()
        };
        let csv = vehicle_csv("ABC1D23", d(1), d(31), &input);
        assert!(csv.starts_with('\u{feff}'));
        assert!(csv.contains("\"'=SUM(A1)\""));
        assert!(csv.contains("\"a, \"\"b\"\"\""));
        assert!(csv.contains("Subtotal,,,3.50"));
        assert_eq!(money(-5), "-0.05");
    }
}
