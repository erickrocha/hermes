use business::gateway::cost_report_gateway::CostReportGateway;
use chrono::{NaiveDate, TimeZone, Utc};
use entity::audit::{AuditUser, run_with_user};
use entity::{
    extra_trip_entity, fuel_entry_entity, garage_service_log_entity, garage_service_model_entity,
    vehicle_entity, vehicle_expense_entity, work_order_entity, work_order_posting_entity,
};
use sea_orm::{DatabaseBackend, MockDatabase};

fn owner() -> AuditUser {
    AuditUser {
        id: 1,
        email: "owner@example.com".into(),
        tenant_id: Some(42),
        enforce_tenant: true,
    }
}

fn at(day: u32) -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 10, day, 12, 0, 0).unwrap()
}

fn order(
    id: i64,
    date: NaiveDate,
    supplier: Option<&str>,
    external: bool,
) -> work_order_entity::Model {
    work_order_entity::Model {
        id,
        uuid: vec![id as u8; 16],
        tenant_id: Some(42),
        vehicle_id: 33,
        opened_at: at(1),
        odometer_km: 10_000.0,
        origin: "Manual".into(),
        checklist_run_id: None,
        maintenance_plan_id: None,
        preventive_plan_id: None,
        service_type: Some("Repair".into()),
        description: format!("Order {id}"),
        responsible: None,
        status: "Completed".into(),
        observation: None,
        external_service: external,
        supplier: supplier.map(str::to_string),
        invoice_number: Some(format!("INV-{id}")),
        invoice_value_cents: Some(2_500),
        invoice_date: Some(date),
        concluded_at: Some(date),
        created_at: at(1),
        created_by: None,
        updated_at: at(1),
        updated_by: None,
    }
}

fn posting(order_id: i64, total: i64) -> work_order_posting_entity::Model {
    work_order_posting_entity::Model {
        id: order_id,
        uuid: vec![order_id as u8; 16],
        tenant_id: Some(42),
        work_order_id: order_id,
        work_order_item_id: None,
        part_id: 8,
        quantity: 1.0,
        unit_value_cents: total,
        total_value_cents: total,
        cost_source: "Informed".into(),
        created_at: at(1),
        created_by: None,
        updated_at: at(1),
        updated_by: None,
    }
}

fn fuel() -> fuel_entry_entity::Model {
    fuel_entry_entity::Model {
        id: 20,
        uuid: vec![20; 16],
        tenant_id: Some(42),
        vehicle_id: 33,
        recorded_at: at(5),
        volume_liters: 100.0,
        value_cents: 15_000,
        odometer_km: Some(10_100.0),
        station: Some("Station".into()),
        full_tank: true,
        origin: "Manual".into(),
        provider_transaction_id: None,
        reported_by_user_id: None,
        odometer_override_note: None,
        provider_confirmed_at: None,
        deleted_at: None,
        unified_into_id: None,
        unification_note: None,
        created_at: at(5),
        created_by: None,
        updated_at: at(5),
        updated_by: None,
    }
}

fn expense() -> vehicle_expense_entity::Model {
    vehicle_expense_entity::Model {
        id: 30,
        uuid: vec![30; 16],
        tenant_id: Some(42),
        vehicle_id: 33,
        category: "Toll".into(),
        competence_period: "2026-10".into(),
        issue_date: NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
        supplier: Some("Acme".into()),
        invoice_number: Some("E-30".into()),
        description: Some("Highway".into()),
        value_cents: 1_000,
        origin: "Manual".into(),
        created_at: at(5),
        created_by: None,
        updated_at: at(5),
        updated_by: None,
    }
}

fn trip(date: NaiveDate, freight: Option<i64>) -> extra_trip_entity::Model {
    extra_trip_entity::Model {
        id: 40,
        uuid: vec![40; 16],
        tenant_id: Some(42),
        order_code: "TRIP-40".into(),
        trip_date: date,
        customer_id: None,
        start_time: None,
        return_date: None,
        return_time: None,
        destination: None,
        origin_city: None,
        stops: None,
        preferred_vehicle_type: None,
        driver_id: None,
        second_driver_id: None,
        vehicle_id: Some(33),
        freight_value_cents: freight,
        payment: None,
        status: "Scheduled".into(),
        origin: None,
        import_batch_id: None,
        imported_at: None,
        replaced_by_id: None,
        created_at: at(5),
        created_by: None,
        updated_at: at(5),
        updated_by: None,
        preparation_skip_authorized_by: None,
        preparation_skip_authorized_at: None,
        preparation_skip_justification: None,
        preparation_skip_vehicle_id: None,
        preparation_skip_trip_date: None,
        preparation_skip_start_time: None,
        preparation_skip_return_date: None,
        preparation_skip_return_time: None,
        preparation_skip_destination: None,
    }
}

fn service_log(id: i64) -> garage_service_log_entity::Model {
    garage_service_log_entity::Model {
        id,
        uuid: vec![id as u8; 16],
        tenant_id: Some(42),
        attendance_id: 3,
        vehicle_id: 33,
        service_model_id: 50,
        name_key: "OIL".into(),
        new_state: "Performed".into(),
        acted_by_user_id: Some(5),
        acted_at: at(5),
        origin: "Manual".into(),
        created_at: at(5),
        created_by: None,
        updated_at: at(5),
        updated_by: None,
    }
}

fn service_model() -> garage_service_model_entity::Model {
    garage_service_model_entity::Model {
        id: 50,
        uuid: vec![50; 16],
        tenant_id: Some(42),
        name: "Oil change".into(),
        name_key: "OIL".into(),
        display_order: 1,
        active: true,
        service_group: "External".into(),
        required_for_departure: false,
        governed_by_tank: false,
        applicability: "All".into(),
        created_at: at(1),
        created_by: None,
        updated_at: at(1),
        updated_by: None,
    }
}

fn vehicle() -> vehicle_entity::Model {
    vehicle_entity::Model {
        id: 33,
        uuid: vec![33; 16],
        tenant_id: Some(42),
        plate: "ABC1D23".into(),
        model: "Bus".into(),
        status: "Active".into(),
        tracker_device_id: None,
        prefix: Some("BUS-33".into()),
        vehicle_type: Some("Bus".into()),
        odometer_km: Some(10_000.0),
        wheel_type: None,
        spare_tire_count: None,
        spare_tire_type: None,
        spare_tire_notes: None,
        tank_capacity_liters: Some(200.0),
        reference_km_per_liter: None,
        garage_tag: None,
        garage_tag_origin: None,
        created_at: at(1),
        created_by: None,
        updated_at: at(1),
        updated_by: None,
    }
}

#[tokio::test]
async fn empty_report_data_returns_empty_costs_and_missing_record_defaults() {
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([Vec::<work_order_entity::Model>::new()])
        .append_query_results([Vec::<fuel_entry_entity::Model>::new()])
        .append_query_results([Vec::<vehicle_expense_entity::Model>::new()])
        .append_query_results([Vec::<extra_trip_entity::Model>::new()])
        .append_query_results([Vec::<garage_service_log_entity::Model>::new()])
        .append_query_results([Vec::<garage_service_model_entity::Model>::new()])
        .append_query_results([Vec::<vehicle_entity::Model>::new()])
        .append_query_results([Vec::<work_order_entity::Model>::new()])
        .append_query_results([Vec::<work_order_entity::Model>::new()])
        .append_query_results([Vec::<work_order_entity::Model>::new()])
        .append_query_results([Vec::<vehicle_entity::Model>::new()])
        .into_connection();
    let gateway = CostReportGateway::new(db);
    let from = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();

    let result = run_with_user(Some(owner()), async {
        let costs = gateway.vehicle_input(33, from, to).await?;
        let plate = gateway.vehicle_plate(33).await?;
        let work_order = gateway.work_order_cost("missing-order".into()).await?;
        let suppliers = gateway.suppliers().await?;
        let lines = gateway.supplier_lines("Acme", from, to).await?;
        Ok::<_, business::domain::business_error::BusinessError>((
            costs, plate, work_order, suppliers, lines,
        ))
    })
    .await
    .unwrap();

    assert!(result.0.work_orders.is_empty());
    assert!(result.0.fuel.is_empty());
    assert!(result.0.expenses.is_empty());
    assert_eq!(result.0.freight_cents, 0);
    assert_eq!(result.0.freight_trips, 0);
    assert!(result.0.services.is_empty());
    assert!(result.1.is_empty());
    assert_eq!(result.2, None);
    assert!(result.3.is_empty());
    assert!(result.4.is_empty());
}

#[tokio::test]
async fn vehicle_and_supplier_reports_combine_period_costs_and_group_service_history() {
    let date = NaiveDate::from_ymd_opt(2026, 10, 5).unwrap();
    let db = MockDatabase::new(DatabaseBackend::MySql)
        .append_query_results([[order(11, date, Some("Acme"), true)]])
        .append_query_results([vec![posting(11, 1_000), posting(11, 2_000)]])
        .append_query_results([[fuel()]])
        .append_query_results([[expense()]])
        .append_query_results([[trip(date, Some(8_000))]])
        .append_query_results([vec![service_log(51), service_log(52)]])
        .append_query_results([[service_model()]])
        .append_query_results([[vehicle()]])
        .append_query_results([[order(11, date, Some("Acme"), true)]])
        .append_query_results([[posting(11, 1_000), posting(11, 2_000)]])
        .append_query_results([vec![
            order(11, date, Some("Acme"), true),
            order(12, date, Some(" acme "), true),
            order(13, date, Some("Beta"), true),
        ]])
        .append_query_results([vec![
            order(11, date, Some("Acme"), true),
            order(12, date, Some(" acme "), true),
            order(13, date, Some("Beta"), true),
        ]])
        .append_query_results([[vehicle()]])
        .into_connection();
    let gateway = CostReportGateway::new(db);
    let from = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
    let to = NaiveDate::from_ymd_opt(2026, 10, 31).unwrap();

    let result = run_with_user(Some(owner()), async {
        let input = gateway.vehicle_input(33, from, to).await?;
        let plate = gateway.vehicle_plate(33).await?;
        let order_cost = gateway.work_order_cost("order-11".into()).await?;
        let suppliers = gateway.suppliers().await?;
        let lines = gateway.supplier_lines("Acme", from, to).await?;
        Ok::<_, business::domain::business_error::BusinessError>((
            input, plate, order_cost, suppliers, lines,
        ))
    })
    .await
    .unwrap();

    assert_eq!(result.0.work_orders[0].postings_cents, 3_000);
    assert_eq!(result.0.work_orders[0].external_cents, 2_500);
    assert_eq!(result.0.fuel[0].description, "Station");
    assert_eq!(result.0.expenses[0].description, "Toll - Acme - Highway");
    assert_eq!(result.0.freight_cents, 8_000);
    assert_eq!(result.0.freight_trips, 1);
    assert_eq!(result.0.services, [("Oil change".into(), 2)]);
    assert_eq!(result.1, "ABC1D23");
    assert_eq!(result.2, Some((3_000, 2_500)));
    assert_eq!(result.3, ["Acme", "Beta"]);
    assert_eq!(result.4.len(), 2);
    assert_eq!(result.4[0].plate, "ABC1D23");
}
