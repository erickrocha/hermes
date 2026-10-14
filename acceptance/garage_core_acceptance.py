#!/usr/bin/env python3
"""Gate 3 acceptance for EPIC-GA-01…06 and GA-08-S01 (catalogue, triage, presence stamps, validity,
required services / call / queue, arrival automation and day-boundary rollover).

Reuses the helpers and fixture isolation (`qa-ga-` prefix) of garage_monitor_acceptance.py.
GA-064 needs the API started with a short rollover loop:

    GARAGE_ROLLOVER_INTERVAL_SECONDS=3 ./target/debug/hermes
"""
import os
import sys
import time
from datetime import datetime, timedelta, timezone

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import garage_monitor_acceptance as b  # noqa: E402

http, rows, one, ek, record = b.http, b.rows, b.one, b.ek, b.record
CHECK_TABLES = ("tenant", "user", "vehicle", "garage_service_model", "garage_attendance",
                "garage_service", "vehicle_presence_event", "garage_call", "garage_service_log", "fuel_entry",
                "tenant_rule_setting", "garage_cycle_state", "garage_release", "garage_release_item",
                "work_order", "work_order_item", "km_evolution", "extra_trip")


def now():
    return datetime.now(timezone.utc).replace(tzinfo=None)


def vehicle(owner, suffix, vehicle_type=None, capacity=None, reference=None):
    body = {"plate": f"QAGA{suffix}", "model": f"qa-ga-{suffix}", "status": "Active"}
    if vehicle_type:
        body["vehicleType"] = vehicle_type
    if capacity is not None:
        body["tankCapacityLiters"] = capacity
    if reference is not None:
        body["referenceKmPerLiter"] = reference
    status, v = http("POST", "/vehicle", body, owner)
    if status != 201:
        raise RuntimeError(f"create vehicle {suffix} failed ({status}): {ek(v)}")
    return v


def service(owner, name, group="External", required=False, tank=False, applicability=None, active=True):
    status, s = http("POST", "/garage-service", {
        "tenantId": b.TENANT_ID, "name": name, "displayOrder": 0, "active": active,
        "serviceGroup": group, "requiredForDeparture": required, "governedByTank": tank,
        "applicability": applicability or {"scope": "all"},
    }, owner)
    return status, s


def presence(admin, v, event, at, source="Tracker"):
    return http("POST", "/vehicle-presence", {
        "vehicleUuid": v["uuid"], "event": event,
        "occurredAt": at.isoformat(timespec="seconds"), "source": source}, admin)


def get_triage(owner, uuid):
    status, a = http("GET", f"/garage-attendance/uuid/{uuid}", token=owner)
    if status != 200:
        raise RuntimeError(f"get triage failed ({status}): {ek(a)}")
    return a


def active_triage(owner, v):
    return http("GET", f"/garage-attendance/active?vehicleUuid={v['uuid']}", token=owner)


def svc(triage, name):
    return next((s for s in triage["services"] if s["name"] == name), None)


def count(table, extra=""):
    return int(one(f"SELECT COUNT(*) FROM {table} WHERE tenant_id={b.TENANT_ID} {extra}"))


def vehicle_id(v):
    return one(f"SELECT id FROM vehicle WHERE uuid=UNHEX('{v['uuid'].replace('-', '')}')")


def run():
    if http("GET", "/api-docs/openapi.json")[0] != 200:
        raise RuntimeError(f"Hermes API is not reachable at {b.BASE}")
    b.cleanup()
    baseline = {t: one(f"SELECT COUNT(*) FROM {t}") for t in CHECK_TABLES}
    try:
        admin, owner = b.seed()
        reader_email = f"{b.TAG}-reader@hermes.test"
        s_reader, _ = http("POST", "/user", {
            "email": reader_email,
            "name": f"{b.TAG}-reader",
            "role": "TenantUser",
            "enabled": True,
            "tenantId": b.TENANT_ID,
        }, owner)
        if s_reader != 201:
            raise RuntimeError(f"create reader failed ({s_reader})")
        reader_hash = one(f"SELECT password FROM user WHERE email='{reader_email}'")
        access_secret = os.environ.get("HERMES_ACCESS_TOKEN_SECRET") or b.load_env()["ACCESS_TOKEN_SECRET"]
        s_reader_invite, _ = http("POST", "/accept-invite", {
            "token": b.invite_token(reader_email, reader_hash, access_secret),
            "newPassword": b.PASSWORD,
        })
        if s_reader_invite != 204:
            raise RuntimeError(f"activate reader failed ({s_reader_invite})")
        reader = b.login(reader_email, b.PASSWORD)

        # ---- EPIC-GA-08-S01: the tenant's single garage geofence ----------------------
        s_default, default_rules = http("GET", "/tenant-rule-settings", token=owner)
        configured_rules = {
            **default_rules,
            "garageBaseLatitude": -23.5,
            "garageBaseLongitude": -46.6,
            "garageBaseRadiusMeters": 120.0,
        }
        s_configure, _ = http("PUT", "/tenant-rule-settings", configured_rules, owner)
        s_read, configured = http("GET", "/tenant-rule-settings", token=owner)
        invalid_rules = {**configured, "garageBaseLatitude": 91.0}
        s_invalid, _ = http("PUT", "/tenant-rule-settings", invalid_rules, owner)
        s_after, after_invalid = http("GET", "/tenant-rule-settings", token=owner)
        record("GA-080", s_default == 200 and default_rules.get("garageBaseRadiusMeters") == 75.0
               and s_configure == 200 and s_read == 200
               and (configured.get("garageBaseLatitude"), configured.get("garageBaseLongitude"), configured.get("garageBaseRadiusMeters")) == (-23.5, -46.6, 120.0)
               and s_invalid == 400 and s_after == 200
               and (after_invalid.get("garageBaseLatitude"), after_invalid.get("garageBaseLongitude"), after_invalid.get("garageBaseRadiusMeters")) == (-23.5, -46.6, 120.0),
               f"default radius {default_rules.get('garageBaseRadiusMeters')}; configured {s_configure}; round-trip {configured.get('garageBaseLatitude')}/{configured.get('garageBaseLongitude')}/{configured.get('garageBaseRadiusMeters')}; invalid latitude {s_invalid}; preserved {s_after}")

        WASH, FUEL = "QA Lavagem Externa", "QA Combustivel"

        # ---- EPIC-GA-01-S01: the catalogue ------------------------------------------
        s_wash, wash = service(owner, WASH, "Internal", required=True)
        s_fuel, fuel = service(owner, FUEL, "External", tank=True)
        s_dup, dup = service(owner, "qa   lavagem, EXTERNA!")
        s_bad, bad = service(owner, "QA Bogus", group="Sideways")
        s_tmp, tmp = service(owner, "QA Temp")
        s_put = http("PUT", f"/garage-service/uuid/{tmp['uuid']}", {**tmp, "active": False}, owner)[0] if s_tmp == 201 else 0
        s_get, listed = http("GET", "/garage-service", token=owner)
        items = listed.get("items", []) if s_get == 200 else []
        names = [x["name"] for x in items]
        tmp_row = next((x for x in items if x["name"] == "QA Temp"), None)
        record("GA-010", s_wash == s_fuel == s_tmp == 201 and s_dup == 409 and s_bad in (400, 422)
               and WASH in names and tmp_row is not None and tmp_row.get("active") is False and s_put == 200,
               f"create={s_wash}/{s_fuel}; normalised duplicate={s_dup}; unknown group={s_bad}; deactivate={s_put}; listed={len(names)}")

        # ---- EPIC-GA-01-S02: applicability -------------------------------------------
        van = vehicle(owner, "VAN", vehicle_type="Van")
        bus = vehicle(owner, "BUS", vehicle_type="Onibus")
        picked = vehicle(owner, "PICK", vehicle_type="Onibus")
        service(owner, "QA Only Vans", applicability={"scope": "vehicleTypes", "values": ["VAN"]})
        service(owner, "QA Only Picked", applicability={"scope": "vehicles", "values": [picked["uuid"]]})
        got = {}
        for key, v in (("van", van), ("bus", bus), ("picked", picked)):
            got[key] = sorted(s["name"] for s in b.open_attendance(owner, v)["services"])
        base = sorted([WASH, FUEL])
        record("GA-011", got["van"] == sorted(base + ["QA Only Vans"]) and got["bus"] == base
               and got["picked"] == sorted(base + ["QA Only Picked"]),
               f"van={got['van']}; bus={got['bus']}; picked={got['picked']}")

        # ---- EPIC-GA-02-S01: open a triage -------------------------------------------
        p = vehicle(owner, "P")
        t = b.open_attendance(owner, p, priority=3)
        s_again, _ = http("POST", "/garage-attendance", {"vehicleUuid": p["uuid"]}, owner)
        s_act, act = active_triage(owner, p)
        marker = one(f"SELECT active_marker FROM garage_attendance WHERE uuid=UNHEX('{t['uuid'].replace('-', '')}')")
        record("GA-020", t["status"] == "Open" and t["origin"] == "Manual" and t.get("manualPriority") == 3
               and len(t["services"]) == 2 and all(s["state"] == "Pending" for s in t["services"])
               and s_again == 409 and s_act == 200 and act["uuid"] == t["uuid"] and marker == "1",
               f"status={t['status']}; origin={t['origin']}; services={len(t['services'])}; second open={s_again}; active lookup={s_act}; marker={marker}")

        # ---- EPIC-GA-02-S02: marking with its own stamps, audit rows, checkout ---------
        w, f = svc(t, WASH), svc(t, FUEL)
        r_perf = http("PUT", f"/garage-attendance/uuid/{t['uuid']}/service/{w['uuid']}", {"state": "Performed"}, owner)[1]
        r_nn = http("PUT", f"/garage-attendance/uuid/{t['uuid']}/service/{f['uuid']}", {"state": "NotNeeded"}, owner)[1]
        r_pend = http("PUT", f"/garage-attendance/uuid/{t['uuid']}/service/{f['uuid']}", {"state": "Pending"}, owner)[1]
        logs = rows(f"SELECT new_state, acted_by_user_id FROM garage_service_log WHERE tenant_id={b.TENANT_ID} "
                    f"AND attendance_id=(SELECT id FROM garage_attendance WHERE uuid=UNHEX('{t['uuid'].replace('-', '')}')) ORDER BY id")
        s_out, out = http("POST", f"/garage-attendance/uuid/{t['uuid']}/checkout", token=owner)
        s_mark_closed = http("PUT", f"/garage-attendance/uuid/{t['uuid']}/service/{w['uuid']}", {"state": "Pending"}, owner)[0]
        s_out2 = http("POST", f"/garage-attendance/uuid/{t['uuid']}/checkout", token=owner)[0]
        s_reopen = http("POST", "/garage-attendance", {"vehicleUuid": p["uuid"]}, owner)[0]
        record("GA-021", r_perf.get("performedAt") and not r_perf.get("markedAt")
               and r_nn.get("markedAt") and r_pend.get("forcedPendingAt")
               and [l[0] for l in logs] == ["Performed", "NotNeeded", "Pending"] and all(l[1] not in ("", "NULL") for l in logs)
               and s_out == 200 and out["status"] == "Finished" and out["releasedAt"] and out.get("manualPriority") in (None, 0)
               and s_mark_closed == 409 and s_out2 == 409 and s_reopen == 201,
               f"stamps performed/marked/forced={bool(r_perf.get('performedAt'))}/{bool(r_nn.get('markedAt'))}/{bool(r_pend.get('forcedPendingAt'))}; "
               f"audit rows={[l[0] for l in logs]}; checkout={s_out} {out.get('status')} priority={out.get('manualPriority')} (absent = zeroed); "
               f"mark closed={s_mark_closed}; checkout again={s_out2}; reopen={s_reopen}")

        # ---- EPIC-GA-02-S03: release with pendencies and a linked work order ---------
        release_vehicle = vehicle(owner, "REL")
        service(owner, "QA Release not needed", applicability={"scope": "vehicles", "values": [release_vehicle["uuid"]]})
        release_triage = b.open_attendance(owner, release_vehicle, priority=2)
        release_performed = svc(release_triage, WASH)
        release_pending = svc(release_triage, FUEL)
        release_not_needed = svc(release_triage, "QA Release not needed")
        b.mark_service(owner, release_triage, release_performed, "Performed")
        b.mark_service(owner, release_triage, release_not_needed, "NotNeeded")
        release_path = f"/garage-attendance/uuid/{release_triage['uuid']}/release-with-pendency"
        missing_justification = http("POST", release_path, {
            "justification": "  ", "odometerKm": 1234,
            "awaitingPartsServiceUuids": [release_pending["uuid"]],
        }, owner)[0]
        missing_odometer = http("POST", release_path, {
            "justification": "Waiting for a replacement part",
            "awaitingPartsServiceUuids": [release_pending["uuid"]],
        }, owner)[0]
        no_partial_release = one(
            f"SELECT COUNT(*) FROM garage_release WHERE attendance_id=(SELECT id FROM garage_attendance "
            f"WHERE uuid=UNHEX('{release_triage['uuid'].replace('-', '')}'))"
        ) == "0" and one(
            f"SELECT COUNT(*) FROM work_order WHERE tenant_id={b.TENANT_ID} AND vehicle_id={vehicle_id(release_vehicle)}"
        ) == "0"
        s_release, released = http("POST", release_path, {
            "justification": "Waiting for a replacement part",
            "odometerKm": 1234,
            "awaitingPartsServiceUuids": [release_pending["uuid"]],
        }, owner)
        release_id = one(
            f"SELECT id FROM garage_release WHERE attendance_id=(SELECT id FROM garage_attendance "
            f"WHERE uuid=UNHEX('{release_triage['uuid'].replace('-', '')}'))"
        )
        release_statuses = rows(
            f"SELECT work_order_status, COUNT(*) FROM garage_release_item WHERE garage_release_id={release_id} "
            f"GROUP BY work_order_status ORDER BY work_order_status"
        ) if release_id else []
        release_order_id = one(
            f"SELECT work_order_id FROM garage_release WHERE id={release_id}"
        ) if release_id else None
        order_status = one(f"SELECT status FROM work_order WHERE id={release_order_id}") if release_order_id else None
        linked_items = one(
            f"SELECT COUNT(*) FROM garage_release_item ri JOIN work_order_item wi "
            f"ON wi.id=ri.work_order_item_id WHERE ri.garage_release_id={release_id}"
        ) if release_id else "0"
        official_reading = one(
            f"SELECT COUNT(*) FROM km_evolution WHERE tenant_id={b.TENANT_ID} AND vehicle_id={vehicle_id(release_vehicle)} "
            f"AND origin='WorkOrder' AND source_entity_id={release_order_id} AND km=1234"
        ) if release_order_id else "0"
        released_triage = get_triage(owner, release_triage["uuid"])
        s_repeat_release = http("POST", release_path, {
            "justification": "Waiting for a replacement part",
            "odometerKm": 1234,
            "awaitingPartsServiceUuids": [release_pending["uuid"]],
        }, owner)[0]
        record("GA-022", missing_justification == 400 and missing_odometer == 400 and no_partial_release
               and s_release == 201 and released.get("justification") == "Waiting for a replacement part"
               and {item[0]: int(item[1]) for item in release_statuses}.get("Resolved") == 1
               and {item[0]: int(item[1]) for item in release_statuses}.get("AwaitingParts") == 1
               and {item[0]: int(item[1]) for item in release_statuses}.get("Cancelled") == 1
               and order_status == "AwaitingParts" and linked_items == "3" and official_reading == "1"
               and released_triage["status"] == "ReleasedWithPendency"
               and released_triage.get("manualPriority") in (None, 0) and s_repeat_release == 409,
               f"missing justification/odometer={missing_justification}/{missing_odometer}; release={s_release}; "
               f"item outcomes={release_statuses}; order={order_status}; linked items={linked_items}; "
               f"official reading={official_reading}; triage={released_triage['status']}; repeat={s_repeat_release}")

        # ---- EPIC-GA-03-S01: presence stamps ----------------------------------------
        r = vehicle(owner, "R")
        n = now()
        s_dep0, dep0 = presence(admin, r, "Departure", n - timedelta(minutes=40))  # first event: always recorded
        s_dep1, dep1 = presence(admin, r, "Departure", n - timedelta(minutes=35))  # a second departure, no arrival between
        s_a1, a1 = presence(admin, r, "Arrival", n - timedelta(minutes=30))
        s_a2, a2 = presence(admin, r, "Arrival", n - timedelta(minutes=20))
        s_d1, d1 = presence(admin, r, "Departure", n - timedelta(minutes=10))
        s_old = presence(admin, r, "Arrival", n - timedelta(minutes=50))[0]
        s_est, _ = presence(admin, r, "Arrival", n - timedelta(minutes=5), source="ScheduleEstimate")
        s_fut = presence(admin, r, "Arrival", n + timedelta(minutes=30))[0]
        s_tenant = presence(owner, r, "Arrival", n - timedelta(minutes=5))[0]
        events = int(one(f"SELECT COUNT(*) FROM vehicle_presence_event WHERE tenant_id={b.TENANT_ID} AND vehicle_id={vehicle_id(r)}"))
        record("GA-030", s_dep0 == 200 and dep0["recorded"] is True and dep1["recorded"] is False and a1["recorded"] is True
               and a2["recorded"] is False and a2["lastArrivalAt"] == a1["lastArrivalAt"]
               and d1["recorded"] is True and d1["away"] is True and s_old == 409
               and s_est == 400 and s_fut == 400 and s_tenant == 403 and events == 3,
               f"first departure recorded={dep0.get('recorded')}; second departure with no arrival between={dep1.get('recorded')}; arrival={a1.get('recorded')}; repeat arrival={a2.get('recorded')}; "
               f"departure away={d1.get('away')}; backwards={s_old}; estimate={s_est}; future={s_fut}; tenant owner={s_tenant}; stored events={events}")

        # ---- EPIC-GA-04-S01: effective state ----------------------------------------
        s_veh = vehicle(owner, "S")
        n = now()
        ts = b.open_attendance(owner, s_veh)
        presence(admin, s_veh, "Arrival", n - timedelta(minutes=2))
        sw = svc(ts, WASH)
        b.mark_service(owner, ts, sw, "NotNeeded")
        presence(admin, s_veh, "Departure", now() + timedelta(minutes=1))
        ts = get_triage(owner, ts["uuid"])
        sw = svc(ts, WASH)
        record("GA-040", sw["state"] == "NotNeeded" and sw["effectiveState"] == "Pending",
               f"stored={sw['state']}; effective={sw['effectiveState']}; reason={sw['effectiveReason']} (not-needed mark consumed by a later departure)")

        low = vehicle(owner, "TLOW", capacity=100.0, reference=1.0)
        high = vehicle(owner, "THIGH", capacity=100.0, reference=1.0)
        b.set_tank_level(owner, low, 80.0)
        b.set_tank_level(owner, high, 20.0)
        res = {}
        for key, v in (("low", low), ("high", high)):
            tr = b.open_attendance(owner, v)
            b.mark_service(owner, tr, svc(tr, FUEL), "Performed")
            res[key] = svc(get_triage(owner, tr["uuid"]), FUEL)
        record("GA-041", res["low"]["effectiveState"] == "Pending" and res["high"]["effectiveState"] == "Performed",
               f"tank 20%: {res['low']['effectiveState']} ({res['low']['effectiveReason']}); tank 80%: {res['high']['effectiveState']} ({res['high']['effectiveReason']})")

        # ---- EPIC-GA-05-S01: required services --------------------------------------
        vv = vehicle(owner, "V")
        tv = b.open_attendance(owner, vv)
        before = get_triage(owner, tv["uuid"])["requiredPending"]
        b.mark_service(owner, tv, svc(tv, WASH), "Performed")
        after = get_triage(owner, tv["uuid"])["requiredPending"]
        record("GA-050", before == [WASH] and after == [],
               f"required pending before={before}; after the required service is performed={after}; desirable fuel service never listed")

        # ---- EPIC-GA-05-S02: manual call --------------------------------------------
        wv = vehicle(owner, "W")
        s_na = http("POST", "/garage-call", {"vehicleUuid": wv["uuid"]}, owner)[0]
        n = now()
        presence(admin, wv, "Arrival", n - timedelta(minutes=30))
        presence(admin, wv, "Departure", n - timedelta(minutes=20))
        s_c1, c1 = http("POST", "/garage-call", {"vehicleUuid": wv["uuid"]}, owner)
        s_c2, c2 = http("POST", "/garage-call", {"vehicleUuid": wv["uuid"]}, owner)
        calls = int(one(f"SELECT COUNT(*) FROM garage_call WHERE tenant_id={b.TENANT_ID} AND vehicle_id={vehicle_id(wv)}"))
        s_g, g = http("GET", f"/garage-call?vehicleUuid={wv['uuid']}", token=owner)
        # an arrival after the call ends it; nothing else does
        presence(admin, wv, "Arrival", now() + timedelta(minutes=1))
        g_after = http("GET", f"/garage-call?vehicleUuid={wv['uuid']}", token=owner)[1]
        xv = vehicle(owner, "X")
        presence(admin, xv, "Arrival", n - timedelta(minutes=30))
        presence(admin, xv, "Departure", n - timedelta(minutes=20))
        http("POST", "/garage-call", {"vehicleUuid": xv["uuid"]}, owner)
        s_cancel = http("POST", "/garage-call/cancel", {"vehicleUuid": xv["uuid"]}, owner)[0]
        g_cancel = http("GET", f"/garage-call?vehicleUuid={xv['uuid']}", token=owner)[1]
        record("GA-051", s_na in (400, 409, 422) and s_c1 == 201 and c1["active"] is True
               and s_c2 in (200, 201) and c2["calledAt"] == c1["calledAt"] and calls == 1
               and s_g == 200 and g["active"] is True and g_after["active"] is False
               and s_cancel in (200, 204) and g_cancel["active"] is False,
               f"call while not away={s_na}; call={s_c1}; repeat={s_c2} same instant={c2.get('calledAt') == c1.get('calledAt')}; rows={calls}; "
               f"in force={g.get('active')}; after later arrival={g_after.get('active')}; cancel={s_cancel} -> {g_cancel.get('active')}")

        # ---- EPIC-GA-05-S03: queue order and fill-the-tank alert ----------------------
        q = {}
        for key, used, prio, trip in (("q1", 20.0, 2, False), ("q2", 20.0, 1, False),
                                      ("q3", 20.0, None, True), ("q4", 40.0, None, False)):
            q[key] = vehicle(owner, key.upper(), capacity=100.0, reference=1.0)
            b.set_tank_level(owner, q[key], used)
            b.open_attendance(owner, q[key], priority=prio)
            if trip:
                b.add_departure(owner, q[key], key, 30)
        status, queue = http("GET", "/garage-attendance/queue", token=owner)
        mine = {q[k]["uuid"]: k for k in q}
        order = [mine[e["vehicleUuid"]] for e in queue if e["vehicleUuid"] in mine] if status == 200 else []
        alert = {mine[e["vehicleUuid"]]: e["fillTankAlert"] for e in queue if e["vehicleUuid"] in mine} if status == 200 else {}
        record("GA-052", status == 200 and order == ["q2", "q1", "q3", "q4"]
               and alert == {"q1": False, "q2": False, "q3": True, "q4": False},
               f"order={order}; alert (80% tank, 80% + trip, 60% tank)={alert}")

        # ---- EPIC-GA-05-S04: authorize skipping one trip's preparation ----------------
        skip_vehicle = vehicle(owner, "SKIP")
        n = now()
        presence(admin, skip_vehicle, "Arrival", n - timedelta(minutes=10))
        skip_triage = active_triage(owner, skip_vehicle)[1]
        b.mark_service(owner, skip_triage, svc(skip_triage, WASH), "Performed")

        def extra_trip_for(v, suffix):
            time.sleep(2)  # MariaDB stores audit timestamps at whole-second precision.
            departure = now() + timedelta(hours=2)
            status, trip = http("POST", "/extra-trip", {
                "orderCode": f"QA-GA-SKIP-{suffix}-{int(time.time())}",
                "tripDate": departure.date().isoformat(),
                "startTime": departure.time().replace(microsecond=0).isoformat(),
                "returnDate": departure.date().isoformat(),
                "returnTime": (departure + timedelta(hours=2)).time().replace(microsecond=0).isoformat(),
                "destination": "QA terminal",
                "vehicleUuid": v["uuid"],
                "status": "Scheduled",
            }, owner)
            return status, trip

        s_skip_trip, skip_trip = extra_trip_for(skip_vehicle, "A")
        skip_path = f"/extra-trip/uuid/{skip_trip['uuid']}/preparation-skip" if s_skip_trip == 201 else ""
        s_skip = http("POST", skip_path, {"justification": "Vehicle is already prepared"}, owner)[0] if skip_path else 0
        s_skip_blank = http("POST", skip_path, {"justification": "  "}, owner)[0] if skip_path else 0
        s_skip_forbidden = http("POST", skip_path, {"justification": "No permission"}, reader)[0] if skip_path else 0
        departure_at = now() + timedelta(seconds=5)
        s_skip_departure = presence(admin, skip_vehicle, "Departure", departure_at)[0]
        after_authorized_trip = get_triage(owner, skip_triage["uuid"])
        time.sleep(2)
        s_other_trip, other_trip = extra_trip_for(skip_vehicle, "B")
        s_return = presence(admin, skip_vehicle, "Arrival", now() + timedelta(seconds=10))[0]
        s_other_departure = presence(admin, skip_vehicle, "Departure", now() + timedelta(seconds=15))[0]
        after_other_trip = get_triage(owner, skip_triage["uuid"])

        edit_vehicle = vehicle(owner, "SKIPEDIT")
        presence(admin, edit_vehicle, "Arrival", now() - timedelta(minutes=10))
        edit_triage = active_triage(owner, edit_vehicle)[1]
        b.mark_service(owner, edit_triage, svc(edit_triage, WASH), "Performed")
        time.sleep(2)
        s_edit_trip, edit_trip = extra_trip_for(edit_vehicle, "C")
        edit_path = f"/extra-trip/uuid/{edit_trip['uuid']}/preparation-skip"
        s_edit_auth = http("POST", edit_path, {"justification": "Reviewed by platform"}, admin)[0]
        s_edit_read, edit_payload = http("GET", f"/extra-trip/uuid/{edit_trip['uuid']}", token=owner)
        edit_payload["destination"] = "Changed terminal"
        s_edit = http("PUT", f"/extra-trip/uuid/{edit_trip['uuid']}", edit_payload, owner)[0]
        s_edit_departure = presence(admin, edit_vehicle, "Departure", now() + timedelta(seconds=5))[0]
        edit_state = svc(get_triage(owner, edit_triage["uuid"]), WASH)
        edit_auth_count = one(
            f"SELECT COUNT(*) FROM extra_trip WHERE id={edit_trip['id']} AND preparation_skip_authorized_by IS NOT NULL"
        )

        import_vehicle = vehicle(owner, "SKIPIMPORT")
        presence(admin, import_vehicle, "Arrival", now() - timedelta(minutes=10))
        import_triage = active_triage(owner, import_vehicle)[1]
        b.mark_service(owner, import_triage, svc(import_triage, WASH), "Performed")
        time.sleep(2)
        s_import_trip, import_trip = extra_trip_for(import_vehicle, "D")
        import_path = f"/extra-trip/uuid/{import_trip['uuid']}/preparation-skip"
        s_import_auth = http("POST", import_path, {"justification": "Reviewed before import"}, owner)[0]
        s_import_read, import_payload = http("GET", f"/extra-trip/uuid/{import_trip['uuid']}", token=owner)
        import_payload["returnDate"] = (
            datetime.fromisoformat(import_payload["returnDate"]).date() + timedelta(days=1)
        ).isoformat()
        s_import, import_result = http("POST", "/extra-trip/import", [import_payload], owner)
        s_import_departure = presence(admin, import_vehicle, "Departure", now() + timedelta(seconds=5))[0]
        import_state = svc(get_triage(owner, import_triage["uuid"]), WASH)
        import_auth_count = one(
            f"SELECT COUNT(*) FROM extra_trip WHERE id={import_trip['id']} AND preparation_skip_authorized_by IS NOT NULL"
        )
        skip_vehicle_id = vehicle_id(skip_vehicle)
        trip_evidence = rows(
            f"SELECT order_code, created_at, preparation_skip_authorized_by, preparation_skip_vehicle_id, vehicle_id "
            f"FROM extra_trip WHERE vehicle_id={skip_vehicle_id} ORDER BY id"
        )
        service_evidence = rows(
            f"SELECT gs.state, gs.performed_at FROM garage_service gs "
            f"JOIN garage_attendance ga ON ga.id=gs.attendance_id WHERE ga.vehicle_id={skip_vehicle_id}"
        )
        presence_evidence = rows(
            f"SELECT kind, occurred_at FROM vehicle_presence_event "
            f"WHERE vehicle_id={skip_vehicle_id} ORDER BY id"
        )
        record("GA-084", s_skip_trip == 201 and s_skip == 201 and s_skip_blank == 400 and s_skip_forbidden == 403
               and s_skip_departure == 200
               and svc(after_authorized_trip, WASH)["effectiveState"] == "Performed"
               and s_other_trip == 201 and s_return == 200 and s_other_departure == 200
               and svc(after_other_trip, WASH)["effectiveState"] == "Pending"
             and svc(after_other_trip, WASH)["effectiveReason"] == "TripMarked"
             and s_edit_trip == 201 and s_edit_auth == 201 and s_edit_read == 200 and s_edit == 200
             and s_edit_departure == 200 and edit_state["effectiveReason"] == "TripMarked" and edit_auth_count == "0"
             and s_import_trip == 201 and s_import_auth == 201 and s_import_read == 200
             and s_import == 200 and import_result["updated"] == 1 and s_import_departure == 200
             and import_state["effectiveReason"] == "TripMarked" and import_auth_count == "0",
             f"trip/auth/blank/non-admin={s_skip_trip}/{s_skip}/{s_skip_blank}/{s_skip_forbidden}; first departure={s_skip_departure}; "
               f"authorized trip state={svc(after_authorized_trip, WASH).get('effectiveState')}; "
               f"different trip={s_other_trip}; return/departure={s_return}/{s_other_departure}; "
               f"different trip state/reason={svc(after_other_trip, WASH).get('effectiveState')}/{svc(after_other_trip, WASH).get('effectiveReason')}; "
             f"edit trip/auth/read/update/departure={s_edit_trip}/{s_edit_auth}/{s_edit_read}/{s_edit}/{s_edit_departure} -> {edit_state.get('effectiveReason')}; "
             f"import trip/auth/read/import/departure={s_import_trip}/{s_import_auth}/{s_import_read}/{s_import}/{s_import_departure} -> {import_state.get('effectiveReason')}; "
               f"trip/service/presence evidence={trip_evidence}/{service_evidence}/{presence_evidence}")

        # ---- EPIC-GA-06-S01: an arrival opens the triage; carry forward ----------------
        y = vehicle(owner, "Y")
        n = now()
        s_pre, _ = active_triage(owner, y)
        presence(admin, y, "Arrival", n - timedelta(minutes=50))
        s_post, ty = active_triage(owner, y)
        record("GA-060", s_pre == 404 and s_post == 200 and ty["origin"] == "ArrivalAtBase" and len(ty["services"]) == 2
               and all(s["state"] == "Pending" for s in ty["services"]),
               f"before arrival={s_pre}; after={s_post}; origin={ty.get('origin')}; services={len(ty.get('services', []))}")

        b.mark_service(owner, ty, svc(ty, WASH), "Performed")
        presence(admin, y, "Departure", now() + timedelta(minutes=1))
        http("POST", f"/garage-attendance/uuid/{ty['uuid']}/checkout", token=owner)
        presence(admin, y, "Arrival", now() + timedelta(minutes=2))
        ty2 = active_triage(owner, y)[1]
        record("GA-061", ty2["uuid"] != ty["uuid"] and ty2["origin"] == "ArrivalAtBase"
               and svc(ty2, WASH)["state"] == "Performed" and svc(ty2, FUEL)["state"] == "Pending",
               f"short absence: wash={svc(ty2, WASH)['state']}; fuel={svc(ty2, FUEL)['state']}")

        z = vehicle(owner, "Z")
        n = now()
        presence(admin, z, "Arrival", n - timedelta(hours=3))
        tz = active_triage(owner, z)[1]
        b.mark_service(owner, tz, svc(tz, WASH), "Performed")
        presence(admin, z, "Departure", n - timedelta(hours=2))
        http("POST", f"/garage-attendance/uuid/{tz['uuid']}/checkout", token=owner)
        presence(admin, z, "Arrival", n - timedelta(minutes=30))
        tz2 = active_triage(owner, z)[1]
        record("GA-062", tz2["uuid"] != tz["uuid"] and svc(tz2, WASH)["state"] == "Pending",
               f"90-minute absence (a trip): wash={svc(tz2, WASH)['state']} (consumed, not carried)")

        # ---- EPIC-GA-06-S02: a provider full tank marks the fuelling service -------------
        def provider_full_tank(v, at, tx):
            b.sql("INSERT INTO fuel_entry (uuid, tenant_id, vehicle_id, recorded_at, volume_liters, value_cents, odometer_km, "
                  "full_tank, origin, provider_transaction_id, created_at, updated_at) VALUES (UNHEX(REPLACE(UUID(),'-','')), "
                  f"{b.TENANT_ID}, {vehicle_id(v)}, '{at:%Y-%m-%d %H:%M:%S}', 100, 10000, 1000, 1, 'CtaSync', '{tx}', NOW(), NOW())")

        fa, fb = vehicle(owner, "FA"), vehicle(owner, "FB")
        n = now()
        provider_full_tank(fa, n - timedelta(minutes=10), f"qa-ga-tx-{int(time.time())}-a")
        provider_full_tank(fb, n - timedelta(minutes=60), f"qa-ga-tx-{int(time.time())}-b")
        presence(admin, fa, "Arrival", n - timedelta(minutes=20))
        presence(admin, fb, "Arrival", n - timedelta(minutes=20))
        tfa, tfb = active_triage(owner, fa)[1], active_triage(owner, fb)[1]
        origin = one("SELECT origin FROM garage_service_log WHERE tenant_id=%d AND vehicle_id=%s ORDER BY id DESC LIMIT 1"
                     % (b.TENANT_ID, vehicle_id(fa)))
        record("GA-063", svc(tfa, FUEL)["state"] == "Performed" and svc(tfa, WASH)["state"] == "Pending"
               and svc(tfb, FUEL)["state"] == "Pending" and origin and origin.startswith("System"),
               f"full tank after the return: fuel={svc(tfa, FUEL)['state']} (log origin '{origin}'), wash={svc(tfa, WASH)['state']}; "
               f"full tank before the return: fuel={svc(tfb, FUEL)['state']}")

        # ---- EPIC-GA-06-S03: the day-boundary job ----------------------------------------
        settings = http("GET", "/tenant-rule-settings", token=owner)[1]
        today = (now() + timedelta(minutes=settings["garageUtcOffsetMinutes"])).date()
        h, l, k = vehicle(owner, "H"), vehicle(owner, "L"), vehicle(owner, "K")
        th = b.open_attendance(owner, h, priority=4)
        b.mark_service(owner, th, svc(th, WASH), "Performed")
        n = now()
        presence(admin, l, "Arrival", n - timedelta(minutes=30))
        presence(admin, k, "Arrival", n - timedelta(minutes=30))
        tl, tk = active_triage(owner, l)[1], active_triage(owner, k)[1]
        presence(admin, l, "Departure", n - timedelta(minutes=10))
        presence(admin, k, "Departure", n - timedelta(minutes=10))
        for tr in (th, tl):
            b.sql(f"UPDATE garage_attendance SET attendance_date = DATE_SUB('{today}', INTERVAL 2 DAY) "
                  f"WHERE uuid=UNHEX('{tr['uuid'].replace('-', '')}')")
        deadline = time.time() + 30
        while time.time() < deadline:
            r_l = get_triage(owner, tl["uuid"])
            if r_l["status"] != "Open":
                break
            time.sleep(1)
        r_h, r_l, r_k = (get_triage(owner, x["uuid"]) for x in (th, tl, tk))
        record("GA-064", r_h["status"] == "Open" and r_h["attendanceDate"] == str(today) and r_h.get("manualPriority") in (None, 0)
               and svc(r_h, WASH)["state"] == "Performed"
               and r_l["status"] == "LeftForOperation" and r_l["releasedAt"] and r_l.get("manualPriority") in (None, 0)
               and r_k["status"] == "Open",
               f"at base, dated 2 days ago: {r_h['status']} re-dated {r_h['attendanceDate']} priority {r_h.get('manualPriority')} wash {svc(r_h, WASH)['state']}; "
               f"away: {r_l['status']}; away but dated today: {r_k['status']}")

        # ---- EPIC-GA-06-S04: hold until a later trip and record auto departure ------
        required_status, _ = service(owner, "QA Auto Required", required=True)
        optional_status, _ = service(owner, "QA Auto Optional")
        if required_status != 201 or optional_status != 201:
            raise RuntimeError(f"create auto-departure services failed: {required_status}/{optional_status}")
        auto_vehicle = vehicle(owner, "AUTO")
        auto_triage = b.open_attendance(owner, auto_vehicle)
        auto_departure = b.add_departure(owner, auto_vehicle, "AUTO", 0.15)
        auto_before = get_triage(owner, auto_triage["uuid"])
        deadline = time.time() + 30
        auto_after = auto_before
        while time.time() < deadline:
            auto_after = get_triage(owner, auto_triage["uuid"])
            if auto_after["status"] != "Open":
                break
            time.sleep(0.25)
        time.sleep(1.2)
        auto_after = get_triage(owner, auto_triage["uuid"])
        auto_attendance_id = one(
            f"SELECT id FROM garage_attendance WHERE uuid=UNHEX('{auto_triage['uuid'].replace('-', '')}')"
        )
        required_before = {
            item["name"] for item in auto_before["services"]
            if item["requiredForDeparture"] and item["effectiveState"] == "Pending"
        }
        automatic_logs = rows(
            f"SELECT model.name, log.new_state, log.origin, log.acted_by_user_id "
            f"FROM garage_service_log log JOIN garage_service_model model "
            f"ON model.id=log.service_model_id WHERE log.tenant_id={b.TENANT_ID} "
            f"AND log.attendance_id={auto_attendance_id} "
            f"AND log.origin='System: automatic departure' ORDER BY model.name"
        )
        required_after = {
            item["name"] for item in auto_after["services"]
            if item["requiredForDeparture"] and item["state"] == "NotDone"
        }
        optional_result = svc(auto_after, "QA Auto Optional")
        reason = auto_after.get("automaticDepartureJustification", "")
        logged_services = {row[0] for row in automatic_logs}

        already_vehicle = vehicle(owner, "ALREADY")
        already_departure = b.add_departure(owner, already_vehicle, "ALREADY", 0.08)
        while now() <= already_departure:
            time.sleep(0.2)
        already_triage = b.open_attendance(owner, already_vehicle)
        time.sleep(1.2)
        already_after = get_triage(owner, already_triage["uuid"])
        record("GA-083", auto_before["status"] == "Open" and auto_after["status"] == "LeftForOperation"
               and auto_after.get("releasedAt") is not None
               and f"viagem extra '{b.TAG}-AUTO'" in reason
               and required_before and required_after == required_before
               and all(name in reason for name in required_before)
               and optional_result is not None and optional_result["state"] == "Pending"
               and len(automatic_logs) == len(required_before) and logged_services == required_before
               and all(row[1] == "NotDone" and row[3] in ("", "NULL") for row in automatic_logs)
               and already_after["status"] == "Open",
               f"before={auto_before['status']}; due departure={auto_after['status']}; reason={reason}; "
               f"required before/after={sorted(required_before)}/{sorted(required_after)}; "
               f"optional={optional_result and optional_result['state']}; "
               f"system logs={automatic_logs}; departure already open at check-in={already_after['status']}")
    finally:
        b.cleanup()
        after = {t: one(f"SELECT COUNT(*) FROM {t}") for t in CHECK_TABLES}
        print(f"CLEANUP baseline={baseline} after={after} clean={baseline == after}")

    passed = sum(1 for r in b.RESULTS.values() if r["status"] == "PASS")
    print(f"{passed}/{len(b.RESULTS)} scenarios PASS")
    return 1 if passed != len(b.RESULTS) else 0


if __name__ == "__main__":
    try:
        sys.exit(run())
    except Exception as error:
        print(f"QA BLOCKED: {type(error).__name__}: {error}", file=sys.stderr)
        try:
            b.cleanup()
        except Exception as cleanup_error:
            print(f"QA CLEANUP ERROR: {cleanup_error}", file=sys.stderr)
        sys.exit(2)
