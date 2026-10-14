#!/usr/bin/env python3
"""Black-box acceptance for the garage tracker-to-presence poller."""
import json
import os
import sys
import time
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import garage_core_acceptance as garage_core  # noqa: E402

b = garage_core.b
http = b.http
one = b.one
record = b.record
MOCK = os.environ.get("HERMES_TRACKING_MOCK_URL", "http://127.0.0.1:8083/control/positions")
DEVICE_ID = 919191
CENTER = (-23.5, -46.6)


def set_position(latitude, longitude, age_seconds=0, valid=True, odometer_meters=None, ignition=True):
    body = [{
        "deviceId": DEVICE_ID,
        "latitude": latitude,
        "longitude": longitude,
        "ageSeconds": age_seconds,
        "valid": valid,
        "ignition": ignition,
    }]
    if odometer_meters is not None:
        body[0]["odometerMeters"] = odometer_meters
    request = urllib.request.Request(
        MOCK,
        data=json.dumps(body).encode(),
        method="POST",
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=5) as response:
        if response.status != 200:
            raise RuntimeError(f"tracking mock update failed: {response.status}")


def wait_for(owner, vehicle_uuid, predicate, timeout=10):
    deadline = time.monotonic() + timeout
    latest = None
    while time.monotonic() < deadline:
        status, latest = http(
            "GET",
            f"/vehicle-presence?vehicleUuid={vehicle_uuid}",
            token=owner,
        )
        if status == 200 and predicate(latest):
            return latest
        time.sleep(0.25)
    return latest


def cycle_state(vehicle_uuid):
    rows = b.rows(
        "SELECT g.reference_odometer_meters,g.last_observed_odometer_meters,"
        "g.last_observed_at,g.open_cycle_started_at,g.open_cycle_distance_meters,"
        "g.last_completed_cycle_distance_meters "
        "FROM garage_cycle_state g JOIN vehicle v ON v.id=g.vehicle_id "
        f"WHERE g.tenant_id={b.TENANT_ID} AND v.uuid=UNHEX('{vehicle_uuid.replace('-', '')}')"
    )
    return rows[0] if rows else None


def run():
    if http("GET", "/api-docs/openapi.json")[0] != 200:
        raise RuntimeError(f"Hermes API is not reachable at {b.BASE}")
    b.cleanup()
    baseline = {table: int(one(f"SELECT COUNT(*) FROM {table}")) for table in garage_core.CHECK_TABLES}
    admin = owner = None
    try:
        admin, owner = b.seed()
        rules_status, rules = http("GET", "/tenant-rule-settings", token=owner)
        rules.update({
            "garageBaseLatitude": CENTER[0],
            "garageBaseLongitude": CENTER[1],
            "garageBaseRadiusMeters": 75.0,
            "garageCycleFallbackDistanceKm": 2.0,
            "garageOdometerMaxSingleJumpKm": 300.0,
        })
        config_status, _ = http("PUT", "/tenant-rule-settings", rules, owner)
        service_status, _ = garage_core.service(owner, "QA Geofence Service", required=True)
        vehicle_status, vehicle = http("POST", "/vehicle", {
            "tenantId": b.TENANT_ID,
            "plate": "QAGAGEO1",
            "model": "qa-geofence",
            "status": "Active",
            "trackerDeviceId": DEVICE_ID,
        }, admin)
        if rules_status != 200 or config_status != 200 or service_status != 201 or vehicle_status != 201:
            raise RuntimeError(f"setup failed: settings {rules_status}/{config_status}, service {service_status}, vehicle {vehicle_status}")

        b.record_km(owner, vehicle, 12_000.0, "2026-10-13T10:00:00")
        plan_status, preventive_plan = http("POST", "/preventive-plan", {
            "vehicleUuid": vehicle["uuid"],
            "planName": "QA annual service",
            "controlType": "Kilometers",
            "intervalKm": 10_000.0,
            "lastServiceKm": 0.0,
        }, owner)
        if plan_status != 201:
            raise RuntimeError(f"preventive plan setup failed: {plan_status}/{b.ek(preventive_plan)}")

        outside = (CENTER[0] + 0.002, CENTER[1])
        inside = (CENTER[0] + 0.0001, CENTER[1])
        set_position(*inside, odometer_meters=50_000_000.0, ignition=False)
        time.sleep(2.2)
        anchored_cycle = cycle_state(vehicle["uuid"])
        set_position(*outside, odometer_meters=50_000_000.0)
        departure = wait_for(owner, vehicle["uuid"], lambda state: state.get("away") is True)
        departure_cycle = cycle_state(vehicle["uuid"])
        departure_count = int(one(f"SELECT COUNT(*) FROM vehicle_presence_event WHERE tenant_id={b.TENANT_ID}"))

        set_position(*outside, odometer_meters=50_001_000.0)
        time.sleep(2.2)
        moving_cycle = cycle_state(vehicle["uuid"])
        repeated_count = int(one(f"SELECT COUNT(*) FROM vehicle_presence_event WHERE tenant_id={b.TENANT_ID}"))
        set_position(*inside, odometer_meters=50_002_000.0)
        arrival = wait_for(owner, vehicle["uuid"], lambda state: state.get("away") is False and state.get("lastArrivalAt"))
        returned_cycle = cycle_state(vehicle["uuid"])
        arrival_count = int(one(f"SELECT COUNT(*) FROM vehicle_presence_event WHERE tenant_id={b.TENANT_ID}"))
        triage_status, triage = garage_core.active_triage(owner, vehicle)
        cycle_distance_meters = one(
            "SELECT g.last_completed_cycle_distance_meters FROM garage_cycle_state g "
            "JOIN vehicle v ON v.id = g.vehicle_id "
            f"WHERE g.tenant_id={b.TENANT_ID} AND v.uuid=UNHEX('{vehicle['uuid'].replace('-', '')}')"
        )
        cycle_distance_value = (
            float(cycle_distance_meters)
            if cycle_distance_meters not in (None, "NULL")
            else None
        )
        cycle_debug = b.rows(
            "SELECT g.reference_odometer_meters,g.last_observed_odometer_meters,"
            "g.open_cycle_started_at,g.open_cycle_start_odometer_meters,"
            "g.open_cycle_distance_meters,g.last_completed_cycle_at "
            "FROM garage_cycle_state g JOIN vehicle v ON v.id=g.vehicle_id "
            f"WHERE g.tenant_id={b.TENANT_ID} AND v.uuid=UNHEX('{vehicle['uuid'].replace('-', '')}')"
        )
        cards_status, cards_response = http("GET", "/garage-attendance/monitor/cards", token=owner)
        card = next((item for item in cards_response.get("cards", []) if item["vehicleUuid"] == vehicle["uuid"]), None)

        set_position(*outside, age_seconds=301, odometer_meters=60_000_000.0)
        time.sleep(1.2)
        stale_count = int(one(f"SELECT COUNT(*) FROM vehicle_presence_event WHERE tenant_id={b.TENANT_ID}"))
        final_state = wait_for(owner, vehicle["uuid"], lambda state: state.get("away") is False, timeout=1)
        persisted_position_columns = b.rows(
            "SELECT TABLE_NAME,COLUMN_NAME FROM information_schema.COLUMNS "
            "WHERE TABLE_SCHEMA=DATABASE() AND COLUMN_NAME IN ('latitude','longitude')"
        )

        presence_checks = {
            "departure": departure is not None and departure.get("away") is True and departure_count == 1,
            "repeat_suppressed": repeated_count == 1,
            "arrival": arrival is not None and arrival.get("away") is False and arrival_count == 2,
            "arrival_triage": triage_status == 200 and triage.get("origin") == "ArrivalAtBase",
            "stale_ignored": stale_count == 2 and final_state is not None and final_state.get("away") is False,
            "provider_positions_not_persisted": not persisted_position_columns,
        }
        card_checks = {
            "plan_created": plan_status == 201,
            "maintenance_status": cards_status == 200 and card is not None and card.get("maintenanceStatus") == "Overdue",
            "cycle_completed": cycle_distance_value == 2_000.0,
            "card_cycle_distance": card is not None and card.get("lastGarageCycleDistanceKm") == 2.0,
        }
        record("GA-081", all(presence_checks.values()),
               f"departure={departure_count}; repeated outside={repeated_count}; arrival={arrival_count}; "
             f"triage={triage_status}/{triage.get('origin')}; stale remains={stale_count}; "
             f"raw position columns={persisted_position_columns}; checks={presence_checks}")
        record("GA-082", all(card_checks.values()),
               f"plan={plan_status}/{card and card.get('maintenanceStatus')}; "
               f"cycle={cycle_distance_meters} m/{card and card.get('lastGarageCycleDistanceKm')} km; "
               f"cycle state anchor/departure/moving/return={anchored_cycle}/{departure_cycle}/{moving_cycle}/{returned_cycle}; "
               f"checks={card_checks}")
    finally:
        try:
            set_position(0.0, 0.0, valid=False)
        except Exception as error:
            print(f"tracking mock cleanup warning: {error}")
        b.cleanup()
        after = {table: int(one(f"SELECT COUNT(*) FROM {table}")) for table in garage_core.CHECK_TABLES}
        print(f"CLEANUP baseline={baseline} after={after} clean={baseline == after}")

    return 1 if any(b.RESULTS.get(scenario, {}).get("status") != "PASS" for scenario in ("GA-081", "GA-082")) or baseline != after else 0


if __name__ == "__main__":
    try:
        sys.exit(run())
    except Exception as error:
        print(f"QA BLOCKED: {type(error).__name__}: {error}", file=sys.stderr)
        try:
            set_position(0.0, 0.0, valid=False)
            b.cleanup()
        except Exception as cleanup_error:
            print(f"QA CLEANUP ERROR: {cleanup_error}", file=sys.stderr)
        sys.exit(2)