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


def set_position(latitude, longitude, age_seconds=0, valid=True):
    body = [{
        "deviceId": DEVICE_ID,
        "latitude": latitude,
        "longitude": longitude,
        "ageSeconds": age_seconds,
        "valid": valid,
    }]
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

        outside = (CENTER[0] + 0.002, CENTER[1])
        inside = (CENTER[0] + 0.0001, CENTER[1])
        set_position(*outside)
        departure = wait_for(owner, vehicle["uuid"], lambda state: state.get("away") is True)
        departure_count = int(one(f"SELECT COUNT(*) FROM vehicle_presence_event WHERE tenant_id={b.TENANT_ID}"))

        time.sleep(2.2)
        repeated_count = int(one(f"SELECT COUNT(*) FROM vehicle_presence_event WHERE tenant_id={b.TENANT_ID}"))
        set_position(*inside)
        arrival = wait_for(owner, vehicle["uuid"], lambda state: state.get("away") is False and state.get("lastArrivalAt"))
        arrival_count = int(one(f"SELECT COUNT(*) FROM vehicle_presence_event WHERE tenant_id={b.TENANT_ID}"))
        triage_status, triage = garage_core.active_triage(owner, vehicle)

        set_position(*outside, age_seconds=301)
        time.sleep(2.2)
        stale_count = int(one(f"SELECT COUNT(*) FROM vehicle_presence_event WHERE tenant_id={b.TENANT_ID}"))
        final_state = wait_for(owner, vehicle["uuid"], lambda state: state.get("away") is False, timeout=1)

        passed = (
            departure is not None
            and departure.get("away") is True
            and departure_count == 1
            and repeated_count == 1
            and arrival is not None
            and arrival.get("away") is False
            and arrival_count == 2
            and triage_status == 200
            and triage.get("origin") == "ArrivalAtBase"
            and stale_count == 2
            and final_state is not None
            and final_state.get("away") is False
        )
        record("GA-081", passed,
               f"departure={departure_count}; repeated outside={repeated_count}; arrival={arrival_count}; "
               f"triage={triage_status}/{triage.get('origin')}; stale remains={stale_count}; state={final_state}")
    finally:
        try:
            set_position(0.0, 0.0, valid=False)
        except Exception as error:
            print(f"tracking mock cleanup warning: {error}")
        b.cleanup()
        after = {table: int(one(f"SELECT COUNT(*) FROM {table}")) for table in garage_core.CHECK_TABLES}
        print(f"CLEANUP baseline={baseline} after={after} clean={baseline == after}")

    return 1 if b.RESULTS.get("GA-081", {}).get("status") != "PASS" or baseline != after else 0


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