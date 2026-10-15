#!/usr/bin/env python3
"""Acceptance for C-026 `EPIC-MT-04` (HRMS-713..., TRM-260...273): planned maintenance that was not carried out.
Scenarios NE-001...NE-012.

Reuses garage_monitor_acceptance.py's helpers and `qa-ga-` fixtures (like maintenance_notification_acceptance.py). Time is
controlled by planning windows in the past and choosing `occurredAt` of the presence events (`POST /vehicle-presence`, platform
administrator); events of one vehicle must go forward in time, so every scenario gets its own vehicle(s). The background job is off in the
test stack (NON_EXECUTION_EVALUATION_INTERVAL_SECONDS=0); passes are triggered through `POST /non-execution/evaluate`.
Set NE_KEEP=1 to leave the fixtures (an inbox with occurrences) behind for non_execution_walkthrough.cjs: the owner is
qa-ga-owner@hermes.test and the mechanic qa-ga-ne-mech@hermes.test, both with b.PASSWORD. Clean up afterwards with
driver_app_acceptance.cleanup().
"""
import os, sys
from datetime import timedelta
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import garage_monitor_acceptance as b  # noqa: E402
import driver_scope_acceptance as d  # noqa: E402
import driver_app_acceptance as da  # noqa: E402

http, record = b.http, b.record
NIL = "00000000-0000-4000-8000-000000000000"
BASE = "/non-execution"
NOW = d.now().replace(microsecond=0)


def ago(**delta):
    return NOW - timedelta(**delta)


def iso(t):
    return t.isoformat(timespec="seconds")


def presence(admin, vehicle, event, at):
    st, p = http("POST", "/vehicle-presence", {"vehicleUuid": vehicle["uuid"], "event": event, "occurredAt": iso(at), "source": "Tracker"}, admin)
    assert st == 200 and p.get("recorded") is True, (event, at, st, p)


def out_since(admin, vehicle, hours):
    """The vehicle is known to be out of the base since `hours` hours ago (it was in for two hours before that)."""
    presence(admin, vehicle, "Arrival", ago(hours=hours + 2))
    presence(admin, vehicle, "Departure", ago(hours=hours))


def order(token, vehicle, label):
    st, wo = http("POST", "/work-order", {"vehicleUuid": vehicle["uuid"], "odometerKm": 10, "description": f"qa-ga-ne {label}"}, token)
    assert st == 201, (label, st, wo)
    return wo


def plan(token, vehicle, start, end, orders):
    st, p = http("POST", "/maintenance-plan", {"vehicleUuid": vehicle["uuid"], "date": start.date().isoformat(), "plannedStart": iso(start),
                                              "plannedEnd": iso(end), "workOrderUuids": [o["uuid"] for o in orders]}, token)
    assert st == 201, (st, p)
    return p


def plan_status(token, p):
    return http("GET", f"/maintenance-plan/uuid/{p['uuid']}", token=token)[1].get("status")


def evaluate(token):
    st, body = http("POST", f"{BASE}/evaluate", token=token)
    assert st == 200, (st, body)
    return body


def inbox(token):
    st, page = http("GET", f"{BASE}?page=0&pageSize=100", token=token)
    return page["items"] if st == 200 else []


def history(token, wo):
    st, body = http("GET", f"{BASE}/work-order/uuid/{wo['uuid']}", token=token)
    return st, (body["workOrder"] if st == 200 else body)


def in_inbox(token, wo):
    for group in inbox(token):
        for row in group["workOrders"]:
            if row["workOrderUuid"] == wo["uuid"]:
                return row
    return None


def occurrences(wo):
    return [r for r in b.rows(f"SELECT o.kind, o.business_key, o.dwell_minutes, o.minutes_in_window, o.arrived_at, o.departed_at, o.justified_at "
                              f"FROM non_execution_occurrence o JOIN non_execution_record r ON r.id=o.record_id JOIN work_order w ON w.id=r.work_order_id "
                              f"WHERE w.uuid=UNHEX(REPLACE('{wo['uuid']}','-','')) ORDER BY o.id")]


def total_occurrences():
    return int(b.one("SELECT COUNT(*) FROM non_execution_occurrence"))


def run():
    da.cleanup()
    keep = os.environ.get("NE_KEEP") == "1"
    try:
        admin, owner = b.seed()
        _, mech = d.make_user(owner, "qa-ga-ne-mech@hermes.test", "Mechanic")
        _, member = d.make_user(owner, "qa-ga-ne-member@hermes.test", "TenantUser")
        _, driver = d.make_user(owner, "qa-ga-ne-driver@hermes.test", "Driver")
        win = (ago(hours=6), ago(hours=4))   # the usual window: six to four hours ago; the 60-minute grace has long passed

        # NE-001 -- leaving the base never concludes a plan; resolving every order does
        v1 = d.vehicle(owner, "NE1")
        a1, b1 = order(owner, v1, "ne1-a"), order(owner, v1, "ne1-b")
        assert http("PUT", f"/work-order/uuid/{a1['uuid']}/conclude", token=owner)[0] == 200
        out_since(admin, v1, 10)
        presence(admin, v1, "Arrival", ago(hours=5, minutes=30)); presence(admin, v1, "Departure", ago(hours=5))
        p1 = plan(owner, v1, *win, [a1, b1])
        r1 = evaluate(owner)
        after_leaving = plan_status(owner, p1)
        occ_a, occ_b = occurrences(a1), occurrences(b1)
        assert http("PUT", f"/work-order/uuid/{b1['uuid']}/conclude", token=owner)[0] == 200
        r2 = evaluate(owner)
        record("NE-001", after_leaving == "NotExecuted" and occ_a == [] and len(occ_b) == 1 and r2["plansConcluded"] >= 1 and plan_status(owner, p1) == "Concluded"
               and r1["occurrencesRecorded"] >= 1 and in_inbox(owner, b1) is None,
               f"the vehicle came and left with one of two orders open: the plan is {after_leaving} (never Concluded), the resolved order has {len(occ_a)} occurrences and the open one {len(occ_b)}; "
               f"after concluding the other order the evaluation concluded {r2['plansConcluded']} plan and it reads {plan_status(owner, p1)}; the history stays ({len(occurrences(b1))} occurrence) and the order left the inbox")

        # NE-002 -- came and left: the real stay, and how much of it was inside the window
        v2, v2b = d.vehicle(owner, "NE2"), d.vehicle(owner, "NE2B")
        o2, o2b = order(owner, v2, "ne2"), order(owner, v2b, "ne2b")
        out_since(admin, v2, 10)
        arr2, dep2 = ago(hours=5, minutes=30), ago(hours=5)
        presence(admin, v2, "Arrival", arr2); presence(admin, v2, "Departure", dep2)
        out_since(admin, v2b, 10)
        arr2b, dep2b = ago(hours=6, minutes=30), ago(hours=5, minutes=30)   # half an hour before the window opens, left an hour... in the window
        presence(admin, v2b, "Arrival", arr2b); presence(admin, v2b, "Departure", dep2b)
        p2, p2b = plan(owner, v2, *win, [o2]), plan(owner, v2b, *win, [o2b])
        evaluate(owner)
        row, rowb = in_inbox(owner, o2), in_inbox(owner, o2b)
        occ, occb = (row or {}).get("occurrences", [{}])[0], (rowb or {}).get("occurrences", [{}])[0]
        record("NE-002", occ.get("kind") == "EnteredAndLeft" and occ.get("arrivedAt") == iso(arr2) and occ.get("departedAt") == iso(dep2)
               and occ.get("dwellMinutes") == 30 and occ.get("minutesInWindow") == 30 and plan_status(owner, p2) == "NotExecuted"
               and occb.get("dwellMinutes") == 60 and occb.get("minutesInWindow") == 30 and occb.get("arrivedAt") == iso(arr2b) and plan_status(owner, p2b) == "NotExecuted",
               f"a visit wholly inside the window: {occ.get('kind')}, arrived {occ.get('arrivedAt')}, left {occ.get('departedAt')}, stay {occ.get('dwellMinutes')} min, {occ.get('minutesInWindow')} in the window, plan {plan_status(owner, p2)}; "
               f"a visit that began half an hour before the window: stay {occb.get('dwellMinutes')} min, {occb.get('minutesInWindow')} in the window")

        # NE-003 -- did not appear: out since before the window, no presence in it, only after the tolerance
        v3, v3b = d.vehicle(owner, "NE3"), d.vehicle(owner, "NE3B")
        o3, o3b = order(owner, v3, "ne3"), order(owner, v3b, "ne3b")
        out_since(admin, v3, 30); out_since(admin, v3b, 30)
        p3, p3b = plan(owner, v3, *win, [o3]), plan(owner, v3b, ago(hours=1, minutes=30), ago(minutes=30), [o3b])   # the second window ended half an hour ago: inside the 60-minute grace
        before = total_occurrences()
        evaluate(owner)
        row3 = in_inbox(owner, o3)
        occ3 = (row3 or {}).get("occurrences", [{}])[0]
        record("NE-003", occ3.get("kind") == "DidNotAppear" and occ3.get("dwellMinutes") is None and occ3.get("arrivedAt") is None and plan_status(owner, p3) == "NotExecuted"
               and in_inbox(owner, o3b) is None and plan_status(owner, p3b) == "Scheduled" and total_occurrences() == before + 1,
               f"out since 30 h, window ended 4 h ago: {occ3.get('kind')} with stay {occ3.get('dwellMinutes')} and plan {plan_status(owner, p3)}; "
               f"window ended 30 min ago (inside the 60-minute tolerance): {len(occurrences(o3b))} occurrences, plan {plan_status(owner, p3b)}")

        # NE-004 -- no evidence, no claim; and the visit takes precedence over the absence
        cases = {}
        for label, setup in {
            "no events": lambda v: None,
            "present when the window opened": lambda v: presence(admin, v, "Arrival", ago(hours=20)),
            "arrived inside the window, still there": lambda v: (out_since(admin, v, 12), presence(admin, v, "Arrival", ago(hours=5))),
            "arrived only after the window": lambda v: (out_since(admin, v, 12), presence(admin, v, "Arrival", ago(hours=3))),
        }.items():
            v = d.vehicle(owner, f"NE4{len(cases)}")
            o = order(owner, v, f"ne4-{len(cases)}")
            setup(v)
            pl = plan(owner, v, *win, [o])
            cases[label] = (o, pl)
        evaluate(owner)
        v4 = d.vehicle(owner, "NE4V")
        o4v = order(owner, v4, "ne4-visit")
        out_since(admin, v4, 12)
        presence(admin, v4, "Arrival", ago(hours=5)); presence(admin, v4, "Departure", ago(hours=4, minutes=40))
        plan(owner, v4, *win, [o4v])
        evaluate(owner)
        silent = {label: (len(occurrences(o)), plan_status(owner, pl)) for label, (o, pl) in cases.items()}
        visit = occurrences(o4v)
        record("NE-004", all(v == (0, "Scheduled") for v in silent.values()) and [r[0] for r in visit] == ["EnteredAndLeft"],
               f"no occurrence and the plan stays Scheduled for {silent}; an out-since-before vehicle that also made a visit across the window gets {[r[0] for r in visit]} (the visit wins)")

        # NE-005 -- idempotent: the same occurrence is never recorded twice
        keys_before = occurrences(o2)
        count_before = total_occurrences()
        again = evaluate(owner)
        presence(admin, v2, "Arrival", ago(hours=2)); presence(admin, v2, "Departure", ago(hours=1))   # oscillating at the edge, outside the window
        osc = evaluate(owner)
        b.sql(f"UPDATE maintenance_plan SET status='Scheduled' WHERE uuid=UNHEX(REPLACE('{p2['uuid']}','-',''))")   # even a plan put back is not repeated
        forced = evaluate(owner)
        platform = evaluate(admin)
        dup = int(b.one("SELECT COUNT(*) FROM (SELECT tenant_id, business_key FROM non_execution_occurrence GROUP BY tenant_id, business_key HAVING COUNT(*) > 1) x"))
        record("NE-005", again["occurrencesRecorded"] == 0 and osc["occurrencesRecorded"] == 0 and forced["occurrencesRecorded"] == 0 and platform["occurrencesRecorded"] == 0
               and total_occurrences() == count_before and occurrences(o2) == keys_before and dup == 0,
               f"re-evaluating adds {again['occurrencesRecorded']}; a new visit outside the window adds {osc['occurrencesRecorded']}; a plan forced back to Scheduled adds {forced['occurrencesRecorded']}; the platform administrator's pass adds {platform['occurrencesRecorded']}; "
               f"rows {count_before} -> {total_occurrences()}, repeated keys {dup}. (The unique index on (tenant_id, business_key) backs it.) Not exercised here: the background loop itself, off in this stack")

        # NE-006 -- one record per order, accumulating occurrences across reschedules
        v6 = d.vehicle(owner, "NE6")
        o6 = order(owner, v6, "ne6")
        out_since(admin, v6, 80)
        p6a = plan(owner, v6, ago(hours=50), ago(hours=48), [o6])
        evaluate(owner)
        presence(admin, v6, "Arrival", ago(hours=5, minutes=30)); presence(admin, v6, "Departure", ago(hours=5))
        p6b = plan(owner, v6, *win, [o6])   # the first plan is NotExecuted, so the order may be scheduled again
        evaluate(owner)
        records6 = int(b.one(f"SELECT COUNT(*) FROM non_execution_record r JOIN work_order w ON w.id=r.work_order_id WHERE w.uuid=UNHEX(REPLACE('{o6['uuid']}','-',''))"))
        kinds6 = [r[0] for r in occurrences(o6)]
        record("NE-006", records6 == 1 and kinds6 == ["DidNotAppear", "EnteredAndLeft"] and plan_status(owner, p6a) == "NotExecuted" and plan_status(owner, p6b) == "NotExecuted"
               and history(owner, o6)[1].get("totalOccurrences") == 2,
               f"{records6} record for the order with the occurrences {kinds6} from two plans ({plan_status(owner, p6a)}, {plan_status(owner, p6b)}); the history says {history(owner, o6)[1].get('totalOccurrences')}")

        # NE-007 -- the inbox: by vehicle, one row per order, only unjustified occurrences, the whole count
        groups = inbox(owner)
        by_plate = {g["vehicle"]["plate"]: g for g in groups}
        row6 = in_inbox(owner, o6)
        shape_ok = all(g["vehicle"]["uuid"] and g["workOrders"] for g in groups) and len(by_plate) == len(groups)
        record("NE-007", shape_ok and {"QAGANE2", "QAGANE6", "QAGANE3"} <= set(by_plate) and row6 and row6["totalOccurrences"] == 2 and len(row6["occurrences"]) == 2
               and all(len(g["workOrders"]) == len({w["workOrderUuid"] for w in g["workOrders"]}) for g in groups) and in_inbox(owner, b1) is None,
               f"{len(groups)} vehicles in the inbox, each once, with one row per order (e.g. {sorted(by_plate)[:4]}); the rescheduled order shows totalOccurrences={row6 and row6['totalOccurrences']} with {row6 and len(row6['occurrences'])} to justify; the resolved order of NE-001 is absent")

        # NE-008 -- justify: the order leaves the inbox only when nothing is left to justify; its status never changes
        first, second = row6["occurrences"][0], row6["occurrences"][1]
        s_empty = http("PUT", f"{BASE}/occurrence/uuid/{first['uuid']}/justify", {"justification": "   "}, mech)[0]
        s_long = http("PUT", f"{BASE}/occurrence/uuid/{first['uuid']}/justify", {"justification": "x" * 1001}, mech)[0]
        s1, j1 = http("PUT", f"{BASE}/occurrence/uuid/{first['uuid']}/justify", {"justification": "  waiting for the part  "}, mech)
        mid = in_inbox(owner, o6)
        s_twice = http("PUT", f"{BASE}/occurrence/uuid/{first['uuid']}/justify", {"justification": "again"}, owner)
        s2 = http("PUT", f"{BASE}/occurrence/uuid/{second['uuid']}/justify", {"justification": "vehicle was on the road"}, owner)[0]
        status_after = http("GET", f"/work-order/uuid/{o6['uuid']}", token=owner)[1]["status"]
        _, hist = history(owner, o6)
        record("NE-008", (s_empty, s_long) == (400, 400) and s1 == 200 and j1["justification"] == "waiting for the part" and j1["justifiedBy"] == "qa-ga-ne-mech@hermes.test" and j1.get("justifiedAt")
               and mid and len(mid["occurrences"]) == 1 and mid["totalOccurrences"] == 2 and s_twice[0] == 409 and b.ek(s_twice[1]) == "NonExecutionAlreadyJustified"
               and s2 == 200 and in_inbox(owner, o6) is None and status_after == "Open" and hist["totalOccurrences"] == 2 and len(hist["occurrences"]) == 2,
               f"empty {s_empty}, 1001 characters {s_long}; the mechanic justifies one ({s1}, by {j1.get('justifiedBy')}): the order stays with {len(mid['occurrences']) if mid else 0} left (total {mid['totalOccurrences'] if mid else '-'}); justifying again {s_twice[0]} {b.ek(s_twice[1])}; "
               f"the second justified {s2}: out of the inbox, the order is still {status_after}, the history keeps {len(hist['occurrences'])} occurrences")

        # NE-009 -- concluding or cancelling takes the order out of the inbox on its own; the history stays
        v9, v9b = d.vehicle(owner, "NE9"), d.vehicle(owner, "NE9B")
        o9, o9b = order(owner, v9, "ne9"), order(owner, v9b, "ne9b")
        out_since(admin, v9, 30); out_since(admin, v9b, 30)
        plan(owner, v9, *win, [o9]); plan(owner, v9b, *win, [o9b])
        evaluate(owner)
        present = in_inbox(owner, o9) is not None and in_inbox(owner, o9b) is not None
        assert http("PUT", f"/work-order/uuid/{o9['uuid']}/conclude", token=owner)[0] == 200
        assert http("PUT", f"/work-order/uuid/{o9b['uuid']}/cancel", token=owner)[0] == 200
        s_h1, h1 = history(owner, o9); s_h2, h2 = history(owner, o9b)
        record("NE-009", present and in_inbox(owner, o9) is None and in_inbox(owner, o9b) is None and s_h1 == 200 and s_h2 == 200
               and h1["totalOccurrences"] == 1 and h2["totalOccurrences"] == 1 and h1["occurrences"][0].get("justifiedAt") is None,
               f"both orders were in the inbox; after concluding one and cancelling the other they are gone without a justification, and the history endpoint still returns {h1['totalOccurrences']} + {h2['totalOccurrences']} occurrence ({s_h1}, {s_h2})")

        # NE-010 -- manual: only with evidence, the dwell measured from the arrival
        v10, v10b, v10c = d.vehicle(owner, "NE10"), d.vehicle(owner, "NE10B"), d.vehicle(owner, "NE10C")
        o10, o10b, o10c = order(owner, v10, "ne10"), order(owner, v10b, "ne10b"), order(owner, v10c, "ne10c")
        out_since(admin, v10, 10)                                           # no visit
        out_since(admin, v10b, 10)
        presence(admin, v10b, "Arrival", ago(hours=7)); presence(admin, v10b, "Departure", ago(hours=5, minutes=40))   # 80 minutes, 20 of them in the window
        out_since(admin, v10c, 10)
        presence(admin, v10c, "Arrival", ago(hours=5, minutes=30)); presence(admin, v10c, "Departure", ago(hours=5, minutes=28))   # only 2 minutes
        plan(owner, v10, *win, [o10]); plan(owner, v10b, *win, [o10b]); plan(owner, v10c, *win, [o10c])   # plans made AFTER the arrivals
        none = http("POST", f"{BASE}/manual", {"workOrderUuid": o10["uuid"], "justification": "driver said it left"}, owner)
        short = http("POST", f"{BASE}/manual", {"workOrderUuid": o10c["uuid"], "justification": "left too soon"}, owner)
        s_ok, ok = http("POST", f"{BASE}/manual", {"workOrderUuid": o10b["uuid"], "justification": "  left before the work started  "}, mech)
        twice = http("POST", f"{BASE}/manual", {"workOrderUuid": o10b["uuid"], "justification": "again"}, mech)
        assert http("PUT", f"/work-order/uuid/{o10['uuid']}/conclude", token=owner)[0] == 200
        closed = http("POST", f"{BASE}/manual", {"workOrderUuid": o10['uuid'], "justification": "too late"}, owner)
        empty = http("POST", f"{BASE}/manual", {"workOrderUuid": o10b["uuid"], "justification": ""}, owner)
        record("NE-010", (none[0], b.ek(none[1])) == (409, "NonExecutionNotEvidenced") and (short[0], b.ek(short[1])) == (409, "NonExecutionNotEvidenced")
               and s_ok == 201 and ok["kind"] == "Manual" and ok["dwellMinutes"] == 80 and ok["minutesInWindow"] == 20 and ok["justification"] == "left before the work started"
               and ok["justifiedBy"] == "qa-ga-ne-mech@hermes.test" and twice[0] == 409 and (closed[0], b.ek(closed[1])) == (409, "NonExecutionWorkOrderNotOpen") and empty[0] == 400,
               f"no visit {none[0]} {b.ek(none[1])}; a 2-minute visit (below the 10-minute minimum) {short[0]}; an 80-minute stay across the window {s_ok}: {ok.get('kind')}, stay {ok.get('dwellMinutes')} min measured from the arrival, "
               f"{ok.get('minutesInWindow')} in the window, justified by {ok.get('justifiedBy')}; repeating it {twice[0]}; a concluded order {closed[0]} {b.ek(closed[1])}; empty text {empty[0]}")

        # NE-011 -- the tenant's settings change the outcomes; invalid values are refused; another tenant is untouched
        s_get, cfg = http("GET", "/tenant-rule-settings", token=owner)
        defaults = (cfg.get("nonExecutionGraceMinutes"), cfg.get("nonExecutionMinDwellMinutes"))
        owner_b, _ = da.second_tenant(admin)
        invalid = [http("PUT", "/tenant-rule-settings", {**cfg, **patch}, owner)[0] for patch in ({"nonExecutionGraceMinutes": -1}, {"nonExecutionMinDwellMinutes": 0})]
        v11a, v11b = d.vehicle(owner, "NE11"), d.vehicle(owner, "NE11B")
        o11a, o11b = order(owner, v11a, "ne11"), order(owner, v11b, "ne11b")
        out_since(admin, v11a, 30)
        out_since(admin, v11b, 10)
        presence(admin, v11b, "Arrival", ago(hours=5, minutes=30)); presence(admin, v11b, "Departure", ago(hours=5, minutes=22))   # 8 minutes
        plan(owner, v11a, ago(hours=1, minutes=30), ago(minutes=30), [o11a]); plan(owner, v11b, *win, [o11b])
        evaluate(owner)
        before_change = (len(occurrences(o11a)), http("POST", f"{BASE}/manual", {"workOrderUuid": o11b["uuid"], "justification": "8 minutes"}, owner)[0])
        s_put, saved = http("PUT", "/tenant-rule-settings", {**cfg, "nonExecutionGraceMinutes": 0, "nonExecutionMinDwellMinutes": 5}, owner)
        evaluate(owner)
        after_change = (len(occurrences(o11a)), http("POST", f"{BASE}/manual", {"workOrderUuid": o11b["uuid"], "justification": "8 minutes"}, owner)[0])
        other = http("GET", "/tenant-rule-settings", token=owner_b)[1]
        http("PUT", "/tenant-rule-settings", cfg, owner)   # back to what it was
        record("NE-011", s_get == 200 and defaults == (60, 10) and invalid == [400, 400] and s_put == 200 and (saved["nonExecutionGraceMinutes"], saved["nonExecutionMinDwellMinutes"]) == (0, 5)
               and before_change == (0, 409) and after_change == (1, 201) and (other["nonExecutionGraceMinutes"], other["nonExecutionMinDwellMinutes"]) == (60, 10),
               f"defaults {defaults}; negative tolerance and a minimum of 0 are refused {invalid}; with the defaults a window that ended 30 min ago gives {before_change[0]} occurrences and an 8-minute visit {before_change[1]}; "
               f"after tolerance 0 / minimum 5: {after_change[0]} occurrence and manual {after_change[1]}; the other tenant still reads {(other['nonExecutionGraceMinutes'], other['nonExecutionMinDwellMinutes'])}")

        # NE-012 -- roles, scope and the contract
        target = in_inbox(owner, o2)["occurrences"][0]
        read = {n: http("GET", BASE, token=t)[0] for n, t in (("Mechanic", mech), ("TenantUser", member), ("TenantOwner", owner), ("SysAdmin", admin))}
        drv = http("GET", BASE, token=driver)
        member_justify = http("PUT", f"{BASE}/occurrence/uuid/{target['uuid']}/justify", {"justification": "x"}, member)
        member_manual = http("POST", f"{BASE}/manual", {"workOrderUuid": o2["uuid"], "justification": "x"}, member)
        mech_eval, member_eval, driver_eval = (http("POST", f"{BASE}/evaluate", token=t) for t in (mech, member, driver))
        owner_eval, admin_eval = http("POST", f"{BASE}/evaluate", token=owner)[0], http("POST", f"{BASE}/evaluate", token=admin)[0]
        cross = (http("GET", f"{BASE}/work-order/uuid/{o2['uuid']}", token=owner_b)[0], http("PUT", f"{BASE}/occurrence/uuid/{target['uuid']}/justify", {"justification": "x"}, owner_b)[0],
                 http("POST", f"{BASE}/manual", {"workOrderUuid": o2["uuid"], "justification": "x"}, owner_b)[0])
        unknown = http("GET", f"{BASE}/work-order/uuid/{NIL}", token=owner_b)[0]
        b_inbox = inbox(owner_b)
        s_mech = http("PUT", f"{BASE}/occurrence/uuid/{target['uuid']}/justify", {"justification": "mechanic may"}, mech)[0]
        spec = http("GET", "/api-docs/openapi.json")[1]
        contract = {(m.upper(), p): spec["paths"][p][m].get("description", "") for p, m in (("/non-execution", "get"), ("/non-execution/work-order/uuid/{uuid}", "get"),
                    ("/non-execution/occurrence/uuid/{uuid}/justify", "put"), ("/non-execution/manual", "post"), ("/non-execution/evaluate", "post"))}
        anonymous = http("GET", BASE)[0]
        record("NE-012", read == {"Mechanic": 200, "TenantUser": 200, "TenantOwner": 200, "SysAdmin": 200} and drv[0] == 403 and drv[1].get("permission") == "non-execution.read"
               and member_justify[0] == 403 and member_justify[1].get("permission") == "non-execution.justify" and member_manual[0] == 403 and member_manual[1].get("permission") == "non-execution.record-manual"
               and s_mech == 200 and mech_eval[0] == 403 and mech_eval[1].get("permission") == "non-execution.evaluate" and member_eval[0] == 403 and driver_eval[0] == 403
               and owner_eval == 200 and admin_eval == 200 and cross == (404, 404, 404) and unknown == 404 and b_inbox == [] and anonymous in (401, 403)
               and all("**Permission:**" in text for text in contract.values()) and "`non-execution.evaluate`" in contract[("POST", "/non-execution/evaluate")],
               f"read {read}; driver {drv[0]} naming {drv[1].get('permission')}; the tenant user cannot justify ({member_justify[0]}, {member_justify[1].get('permission')}) or record ({member_manual[0]}); the mechanic justifies {s_mech}; "
               f"evaluate: mechanic {mech_eval[0]} naming {mech_eval[1].get('permission')}, tenant user {member_eval[0]}, driver {driver_eval[0]}, owner {owner_eval}, platform {admin_eval}; other tenant on A's order/occurrence/manual {cross} (an unknown uuid {unknown}), its inbox {len(b_inbox)}; "
               f"every new operation states **Permission:** in the contract ({len(contract)}); no token {anonymous}")
        if keep:   # leave the walkthrough's raw material: an inbox for the console
            vk = d.vehicle(owner, "NEK")
            ok_ = order(owner, vk, "nek-ui")
            out_since(admin, vk, 30)
            plan(owner, vk, *win, [ok_])
            evaluate(owner)
    finally:
        if not keep:
            da.cleanup()
    bad = [k for k, v in b.RESULTS.items() if v["status"] != "PASS"]
    print(f"{len(b.RESULTS) - len(bad)}/{len(b.RESULTS)} scenarios PASS")
    return 1 if bad else 0


if __name__ == "__main__":
    try:
        sys.exit(run())
    except Exception as error:
        print(f"QA BLOCKED: {type(error).__name__}: {error}", file=sys.stderr)
        try:
            da.cleanup()
        except Exception as e2:
            print(f"cleanup: {e2}", file=sys.stderr)
        sys.exit(2)
