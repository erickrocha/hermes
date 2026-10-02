#!/usr/bin/env python3
"""Gate 3 acceptance for EPIC-GA-07 (garage monitor and call list).

Uses the running Hermes API and its dev MariaDB. Fixtures are isolated by the
`qa-ga-` prefix and deleted in dependency order on exit.
"""
import base64
import hashlib
import hmac
import json
import os
import re
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timedelta, timezone

HERE = os.path.dirname(os.path.abspath(__file__))
BACKEND = os.path.join(os.path.dirname(HERE), "backend")
BASE = os.environ.get("HERMES_API", "http://127.0.0.1:8080")
DB_CONTAINER = os.environ.get("HERMES_DB_CONTAINER", "dev-mariadb-1")
DB_USER = os.environ.get("HERMES_DB_USER", "hermes")
DB_PASS = os.environ.get("HERMES_DB_PASSWORD", "brutal")
DB_NAME = os.environ.get("HERMES_DB_NAME", "hermes")
ADMIN_EMAIL = os.environ.get("HERMES_ADMIN_EMAIL", "admin@hermes.dev")
TAG = "qa-ga"
PASSWORD = "QaGarageMonitor#2026"
RESULTS = {}
TENANT_ID = None


def record(scenario, passed, evidence):
    status = "PASS" if passed else "FAIL"
    RESULTS[scenario] = {"status": status, "evidence": evidence}
    print(f"{scenario:8} {status:5} {evidence}", flush=True)


def http(method, path, body=None, token=None, form=None):
    headers = {}
    data = None
    if form is not None:
        data = urllib.parse.urlencode(form).encode()
        headers["Content-Type"] = "application/x-www-form-urlencoded"
    elif body is not None:
        data = json.dumps(body).encode()
        headers["Content-Type"] = "application/json"
    if token:
        headers["Authorization"] = f"Bearer {token}"
    request = urllib.request.Request(BASE + path, data=data, method=method, headers=headers)
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            status, raw = response.status, response.read()
    except urllib.error.HTTPError as error:
        status, raw = error.code, error.read()
    except (urllib.error.URLError, TimeoutError, ConnectionError) as error:
        return 0, str(error)
    try:
        return status, json.loads(raw) if raw else None
    except ValueError:
        return status, raw.decode(errors="replace")


def sql(statement, check=True):
    process = subprocess.run(
        ["docker", "exec", "-i", DB_CONTAINER, "mariadb", f"-u{DB_USER}", f"-p{DB_PASS}", "-N", "-B", DB_NAME],
        input=statement,
        capture_output=True,
        text=True,
    )
    if check and process.returncode:
        raise RuntimeError(process.stderr.strip())
    return process


def rows(statement):
    result = sql(statement)
    return [line.split("\t") for line in result.stdout.splitlines()]


def one(statement):
    result = rows(statement)
    return result[0][0] if result else None


def load_env():
    values = {}
    with open(os.path.join(BACKEND, ".env"), encoding="utf-8") as source:
        for line in source:
            match = re.match(r"^\s*([A-Z_]+)=(.*)$", line)
            if match:
                values[match.group(1)] = match.group(2).strip().strip('"')
    return values


def b64(value):
    return base64.urlsafe_b64encode(value).rstrip(b"=").decode()


def invite_token(email, password_hash, secret):
    header = b64(json.dumps({"typ": "JWT", "alg": "HS512"}).encode())
    claims = b64(json.dumps({"sub": email, "exp": int(time.time()) + 600, "typ": "invite"}).encode())
    key = f"{secret}:invite:{password_hash}".encode()
    signature = b64(hmac.new(key, f"{header}.{claims}".encode(), hashlib.sha512).digest())
    return f"{header}.{claims}.{signature}"


def ek(payload):
    return payload.get("errorKey") if isinstance(payload, dict) else payload


def login(email, password):
    status, payload = http("POST", "/login", form={"email": email, "password": password})
    if status != 200:
        raise RuntimeError(f"login failed ({status}): {ek(payload)}")
    return payload["accessToken"]


def cleanup():
    tenant_query = f"SELECT id FROM tenant WHERE business_name LIKE '{TAG}-%'"
    tenant_ids = ",".join(row[0] for row in rows(tenant_query)) or "0"
    clauses = f"tenant_id IN ({tenant_ids})"
    for table in ("garage_service_log", "garage_call", "garage_service", "garage_attendance",
                  "vehicle_presence_event", "extra_trip", "fuel_entry", "km_evolution",
                  "tenant_rule_setting"):
        sql(f"DELETE FROM {table} WHERE {clauses}")
    sql(f"DELETE FROM garage_service_model WHERE {clauses}")
    sql(f"DELETE FROM vehicle WHERE {clauses}")
    sql(f"DELETE FROM user WHERE email LIKE '{TAG}-%@hermes.test'")
    sql(f"DELETE FROM tenant WHERE id IN ({tenant_ids})")


def seed():
    global TENANT_ID
    config = load_env()
    admin_password = os.environ.get("HERMES_ADMIN_PASSWORD") or config.get("SYSADMIN_PASSWORD", "")
    admin = login(ADMIN_EMAIL, admin_password)

    tax_id = f"QAGA{int(time.time()) % 100000000:08d}"
    status, tenant = http("POST", "/tenant", {
        "businessName": f"{TAG}-tenant",
        "taxId": tax_id,
        "countryCode": "US",
    }, admin)
    if status != 201:
        raise RuntimeError(f"create tenant failed ({status}): {ek(tenant)}")
    TENANT_ID = int(tenant["id"])

    email = f"{TAG}-owner@hermes.test"
    status, user = http("POST", "/user", {
        "email": email,
        "name": f"{TAG}-owner",
        "role": "TenantOwner",
        "enabled": True,
        "tenantId": TENANT_ID,
    }, admin)
    if status != 201:
        raise RuntimeError(f"create owner failed ({status}): {ek(user)}")
    stored = rows(f"SELECT password FROM user WHERE email='{email}'")
    status, accepted = http("POST", "/accept-invite", {
        "token": invite_token(email, stored[0][0], config["ACCESS_TOKEN_SECRET"]),
        "newPassword": PASSWORD,
    })
    if status != 204:
        raise RuntimeError(f"activate owner failed ({status}): {ek(accepted)}")
    return admin, login(email, PASSWORD)


def create_vehicle(owner, suffix, capacity=None, reference=None):
    body = {"plate": f"QAGA{suffix}", "model": f"{TAG}-{suffix}", "status": "Active"}
    if capacity is not None:
        body["tankCapacityLiters"] = capacity
    if reference is not None:
        body["referenceKmPerLiter"] = reference
    status, vehicle = http("POST", "/vehicle", body, owner)
    if status != 201:
        raise RuntimeError(f"create vehicle {suffix} failed ({status}): {ek(vehicle)}")
    return vehicle


def record_km(owner, vehicle, km, recorded_at):
    status, result = http("POST", f"/vehicle/uuid/{vehicle['uuid']}/km-evolution", {
        "km": km,
        "recordedAt": recorded_at,
        "origin": "Manual",
        "notes": "garage monitor acceptance fixture",
    }, owner)
    if status != 201:
        raise RuntimeError(f"record odometer failed ({status}): {ek(result)}")


def record_full_fuel(owner, vehicle, odometer, recorded_at):
    status, result = http("POST", "/fuel-entry", {
        "vehicleUuid": vehicle["uuid"],
        "recordedAt": recorded_at,
        "volumeLiters": 100.0,
        "valueCents": 10000,
        "odometerKm": odometer,
        "fullTank": True,
        "station": "QA fixture",
    }, owner)
    if status != 201:
        raise RuntimeError(f"record fuel failed ({status}): {ek(result)}")


def set_tank_level(owner, vehicle, used_km):
    now = datetime.now(timezone.utc).replace(tzinfo=None)
    anchor = now - timedelta(hours=3)
    record_full_fuel(owner, vehicle, 1000.0, anchor.isoformat(timespec="seconds"))
    record_km(owner, vehicle, 1000.0 + used_km, now.isoformat(timespec="seconds"))


def add_departure(owner, vehicle, suffix, minutes_from_now=30):
    departure = datetime.now(timezone.utc).replace(tzinfo=None) + timedelta(minutes=minutes_from_now)
    status, trip = http("POST", "/extra-trip", {
        "orderCode": f"{TAG}-{suffix}",
        "tripDate": departure.date().isoformat(),
        "startTime": departure.time().replace(microsecond=0).isoformat(),
        "vehicleUuid": vehicle["uuid"],
        "status": "Scheduled",
        "destination": "QA destination",
    }, owner)
    if status != 201:
        raise RuntimeError(f"create departure failed ({status}): {ek(trip)}")
    return departure


def publish_presence(admin, vehicle, event, at):
    status, result = http("POST", "/vehicle-presence", {
        "vehicleUuid": vehicle["uuid"],
        "event": event,
        "occurredAt": at.isoformat(timespec="seconds"),
        "source": "Tracker",
    }, admin)
    if status != 200 or not result.get("recorded"):
        raise RuntimeError(f"presence {event} failed ({status}): {ek(result)}")


def open_attendance(owner, vehicle, priority=None):
    body = {"vehicleUuid": vehicle["uuid"]}
    if priority is not None:
        body["manualPriority"] = priority
    status, attendance = http("POST", "/garage-attendance", body, owner)
    if status != 201:
        raise RuntimeError(f"open triage failed ({status}): {ek(attendance)}")
    return attendance


def create_catalogue(owner):
    services = {}
    for key, name, required, tank_governed in (
        ("fuel", "QA fuel service", False, True),
        ("wash", "QA wash service", True, False),
    ):
        status, service = http("POST", "/garage-service", {
            "tenantId": TENANT_ID,
            "name": name,
            "displayOrder": len(services),
            "active": True,
            "serviceGroup": "External" if key == "fuel" else "Internal",
            "requiredForDeparture": required,
            "governedByTank": tank_governed,
            "applicability": {"scope": "all"},
        }, owner)
        if status != 201:
            raise RuntimeError(f"create service {key} failed ({status}): {ek(service)}")
        services[key] = service
    return services


def configure_rules(owner):
    status, settings = http("GET", "/tenant-rule-settings", token=owner)
    if status != 200:
        raise RuntimeError(f"get tenant rules failed ({status}): {ek(settings)}")
    settings["garageMonitorCardLimit"] = 1
    status, saved = http("PUT", "/tenant-rule-settings", settings, owner)
    if status != 200:
        raise RuntimeError(f"configure tenant rules failed ({status}): {ek(saved)}")


def mark_service(owner, attendance, service, state):
    status, result = http(
        "PUT",
        f"/garage-attendance/uuid/{attendance['uuid']}/service/{service['uuid']}",
        {"state": state},
        owner,
    )
    if status != 200:
        raise RuntimeError(f"mark service failed ({status}): {ek(result)}")
    return result


def run():
    if http("GET", "/api-docs/openapi.json")[0] != 200:
        raise RuntimeError(f"Hermes API is not reachable at {BASE}")
    cleanup()
    baseline = {
        table: one(f"SELECT COUNT(*) FROM {table}")
        for table in ("tenant", "user", "vehicle", "garage_service_model", "garage_attendance", "garage_service")
    }
    admin = owner = None
    try:
        admin, owner = seed()
        configure_rules(owner)
        catalog = create_catalogue(owner)

        monitor_vehicle = create_vehicle(owner, "MON")
        monitor_attendance = open_attendance(owner, monitor_vehicle, priority=1)
        departure = add_departure(owner, monitor_vehicle, "MON", 30)
        status, monitor = http("GET", "/garage-attendance/monitor", token=owner)
        monitor_entry = next((entry for entry in monitor if entry.get("vehicleUuid") == monitor_vehicle["uuid"]), None) if status == 200 else None
        record("GA-070", status == 200 and monitor_entry is not None
               and monitor_entry.get("readiness") == "Alert" and monitor_entry.get("urgent") is True
               and monitor_entry.get("attendanceUuid") == monitor_attendance["uuid"],
               f"monitor departure={departure.isoformat(timespec='minutes')}; readiness={monitor_entry and monitor_entry.get('readiness')}; urgent={monitor_entry and monitor_entry.get('urgent')}")

        low_vehicle = create_vehicle(owner, "LOW", capacity=100.0, reference=1.0)
        high_vehicle = create_vehicle(owner, "HIGH", capacity=100.0, reference=1.0)
        set_tank_level(owner, low_vehicle, 70.0)
        set_tank_level(owner, high_vehicle, 20.0)
        open_attendance(owner, low_vehicle)
        open_attendance(owner, high_vehicle)
        add_departure(owner, high_vehicle, "HIGH", 40)
        status, cards = http("GET", "/garage-attendance/monitor/cards", token=owner)
        card_ids = [card.get("vehicleUuid") for card in cards.get("cards", [])] if status == 200 else []
        card_low = next((c for c in cards.get("cards", []) if c.get("vehicleUuid") == low_vehicle["uuid"]), None) if status == 200 else None
        record("GA-071", status == 200 and card_ids == [low_vehicle["uuid"]]
               and cards.get("notShown") == 2 and card_low is not None
               and card_low.get("needsRefuel") is True and abs(card_low.get("tankPercent", 0) - 30.0) < 0.01,
               f"cards={card_ids}; hidden={cards.get('notShown') if status == 200 else None}; low tank={card_low and card_low.get('tankPercent')}")

        call_vehicle = create_vehicle(owner, "CALL")
        stamp = datetime.now(timezone.utc).replace(tzinfo=None)
        publish_presence(admin, call_vehicle, "Arrival", stamp - timedelta(minutes=10))
        publish_presence(admin, call_vehicle, "Departure", stamp - timedelta(minutes=5))
        status, manual = http("POST", "/garage-call", {"vehicleUuid": call_vehicle["uuid"]}, owner)
        status_list, call_entries = http("GET", "/garage-attendance/call-list", token=owner)
        manual_entry = next((e for e in call_entries if e.get("vehicleUuid") == call_vehicle["uuid"]), None) if status_list == 200 else None
        record("GA-072", status == 201 and manual.get("active") is True and status_list == 200
               and manual_entry is not None and manual_entry.get("calledByManager") is True
               and manual_entry.get("hasDepartureAhead") is False,
               f"manual call status={status}; listed={manual_entry is not None}; reason={manual_entry}")

        suppress_vehicle = create_vehicle(owner, "SUP", capacity=100.0, reference=1.0)
        floor_vehicle = create_vehicle(owner, "FLOOR", capacity=100.0, reference=1.0)
        set_tank_level(owner, suppress_vehicle, 20.0)
        set_tank_level(owner, floor_vehicle, 70.0)
        add_departure(owner, suppress_vehicle, "SUP", 50)
        add_departure(owner, floor_vehicle, "FLOOR", 50)
        for vehicle in (suppress_vehicle, floor_vehicle):
            at = datetime.now(timezone.utc).replace(tzinfo=None)
            attendance = open_attendance(owner, vehicle)
            fuel_service = next(s for s in attendance["services"] if s.get("serviceModelUuid") == catalog["fuel"]["uuid"])
            mark_service(owner, attendance, fuel_service, "Performed")
            publish_presence(admin, vehicle, "Arrival", at - timedelta(minutes=10))
            publish_presence(admin, vehicle, "Departure", at - timedelta(minutes=5))
        status, call_entries = http("GET", "/garage-attendance/call-list", token=owner)
        suppress_entry = next((e for e in call_entries if e.get("vehicleUuid") == suppress_vehicle["uuid"]), None) if status == 200 else None
        floor_entry = next((e for e in call_entries if e.get("vehicleUuid") == floor_vehicle["uuid"]), None) if status == 200 else None
        record("GA-073", status == 200 and suppress_entry is not None and suppress_entry.get("hasDepartureAhead") is True
               and suppress_entry.get("tankCalls") is False and floor_entry is not None and floor_entry.get("tankCalls") is True,
               f"resolved fuel above floor: tankCalls={suppress_entry and suppress_entry.get('tankCalls')}; critical floor: tankCalls={floor_entry and floor_entry.get('tankCalls')}")

        wash_record = next(
            service for service in monitor_attendance["services"]
            if service.get("serviceModelUuid") == catalog["wash"]["uuid"]
        )
        mark_service(owner, monitor_attendance, wash_record, "Performed")
        status, late_service = http("POST", "/garage-service", {
            "tenantId": TENANT_ID,
            "name": "QA late service",
            "displayOrder": 2,
            "active": True,
            "serviceGroup": "External",
            "requiredForDeparture": False,
            "governedByTank": False,
            "applicability": {"scope": "all"},
        }, owner)
        if status != 201:
            raise RuntimeError(f"create late service failed ({status}): {ek(late_service)}")
        status, matrix = http("GET", "/garage-attendance/monitor/matrix", token=owner)
        matrix_row = next((r for r in matrix.get("rows", []) if r.get("vehicleUuid") == monitor_vehicle["uuid"]), None) if status == 200 else None
        col_names = [c.get("name") for c in matrix.get("columns", [])] if status == 200 else []
        cells = matrix_row.get("cells", []) if matrix_row else []
        record("GA-074", status == 200 and matrix_row is not None
               and col_names == [catalog["fuel"]["name"], late_service["name"], catalog["wash"]["name"]]
               and len(cells) == 3 and cells[0] is not None and cells[0].get("effectiveState") == "Pending"
               and cells[1] is None and cells[2] is not None and cells[2].get("effectiveState") == "Performed",
               f"columns={col_names}; row cells={matrix_row and matrix_row.get('cells')}")
    finally:
        cleanup()
        after = {
            table: one(f"SELECT COUNT(*) FROM {table}")
            for table in ("tenant", "user", "vehicle", "garage_service_model", "garage_attendance", "garage_service")
        }
        print(f"CLEANUP baseline={baseline} after={after}")

    for scenario, result in RESULTS.items():
        print(f"{scenario} {result['status']} {result['evidence']}")
    return 1 if any(result["status"] != "PASS" for result in RESULTS.values()) else 0


if __name__ == "__main__":
    try:
        sys.exit(run())
    except Exception as error:
        print(f"QA BLOCKED: {type(error).__name__}: {error}", file=sys.stderr)
        try:
            cleanup()
        except Exception as cleanup_error:
            print(f"QA CLEANUP ERROR: {cleanup_error}", file=sys.stderr)
        sys.exit(2)