#!/usr/bin/env python3
"""Acceptance for C-026 `EPIC-MT-06` (HRMS-706, TRM-616): the in-app notice "a part you were waiting for has arrived".
Scenarios MN-001...MN-006.

Reuses garage_monitor_acceptance.py's helpers and `qa-ga-` fixtures (like driver_scope_acceptance.py / driver_app_acceptance.py).
Set MN_KEEP=1 to leave the fixtures (and three unread notices) behind for maintenance_notification_walkthrough.cjs; the owner
is qa-ga-owner@hermes.test and the mechanic qa-ga-mn-mech@hermes.test, both with b.PASSWORD. Clean up afterwards with
driver_app_acceptance.cleanup().
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import garage_monitor_acceptance as b  # noqa: E402
import driver_scope_acceptance as d  # noqa: E402
import driver_app_acceptance as da  # noqa: E402

http, record = b.http, b.record
NIL = "00000000-0000-4000-8000-000000000000"
BASE = "/maintenance-notification"


def part(token, label):
    st, p = http("POST", "/part", {"name": f"qa-ga-{label}", "unit": "un", "minimumStock": 0}, token)
    assert st == 201, (label, st, p)
    return p


def receive(token, p, qty, work_order=None):
    """Raise a purchase order (linked to a pendency of `work_order`, if any), order it and receive it; returns the last status."""
    body = {"partUuid": p["uuid"], "quantity": qty}
    if work_order:
        st, item = http("POST", f"/work-order/uuid/{work_order['uuid']}/item", {"description": "needs the part", "status": "Pending"}, token)
        assert st == 201, (st, item)
        items = http("GET", f"/work-order/uuid/{work_order['uuid']}", token=token)[1]["items"]
        body.update(workOrderUuid=work_order["uuid"], workOrderItemUuid=items[0]["uuid"])
    st, order = http("POST", "/purchase-order", body, token)
    assert st == 201, (st, order)
    assert http("PUT", f"/purchase-order/uuid/{order['uuid']}/mark-ordered", {}, token)[0] == 200
    return http("PUT", f"/purchase-order/uuid/{order['uuid']}/mark-purchased", token=token)[0]


def listing(token, query=""):
    st, page = http("GET", f"{BASE}?page=0&pageSize=50{query}", token=token)
    return st, (page["items"] if st == 200 else [])


def count(token):
    st, body = http("GET", f"{BASE}/unread-count", token=token)
    return body["count"] if st == 200 else None


def db_count():
    return int(b.one("SELECT COUNT(*) FROM maintenance_notification"))


def run():
    da.cleanup()
    keep = os.environ.get("MN_KEEP") == "1"
    try:
        admin, owner = b.seed()
        mech_user, mech = d.make_user(owner, "qa-ga-mn-mech@hermes.test", "Mechanic")
        _, member = d.make_user(owner, "qa-ga-mn-member@hermes.test", "TenantUser")
        _, driver = d.make_user(owner, "qa-ga-mn-driver@hermes.test", "Driver")
        vehicle = d.vehicle(owner, "MN1")
        st, wo1 = http("POST", "/work-order", {"vehicleUuid": vehicle["uuid"], "odometerKm": 10, "description": "qa-ga-mn brakes"}, owner)
        assert st == 201, (st, wo1)
        st, wo2 = http("POST", "/work-order", {"vehicleUuid": vehicle["uuid"], "odometerKm": 20, "description": "qa-ga-mn filter"}, owner)
        assert st == 201, (st, wo2)
        pads, filt, loose = part(owner, "mn-pads"), part(owner, "mn-filter"), part(owner, "mn-loose")

        # MN-001 -- receiving a purchase order linked to a work order raises exactly one notice, with the part and the quantity
        before = db_count()
        s_recv = receive(mech, pads, 4.0, wo1)
        s_list, items = listing(owner)
        n = items[0] if items else {}
        again = http("PUT", f"/purchase-order/uuid/{http('GET', '/purchase-order?page=0&pageSize=50', token=owner)[1]['items'][0]['uuid']}/mark-purchased", token=mech)[0]
        record("MN-001", s_recv == 200 and s_list == 200 and len(items) == 1 and db_count() == before + 1 and n.get("partName") == "qa-ga-mn-pads"
               and n.get("quantity") == 4.0 and n.get("workOrderUuid") == wo1["uuid"] and n.get("partUuid") == pads["uuid"] and n.get("read") is False
               and again == 400,
               f"the mechanic receives the order {s_recv}; the owner's inbox has {len(items)} notice ({n.get('partName')}, quantity {n.get('quantity')}, on the work order: "
               f"{n.get('workOrderUuid') == wo1['uuid']}, unread: {n.get('read') is False}); {db_count() - before} row written; receiving it again {again} adds none")

        # MN-002 -- an order with no work order raises nothing
        s_loose = receive(owner, loose, 2.0)
        record("MN-002", s_loose == 200 and db_count() == before + 1 and len(listing(owner)[1]) == 1,
               f"an order with no work order is received {s_loose}; the table still holds {db_count() - before} new notice and the inbox {len(listing(owner)[1])}")

        # MN-003 -- list, filter and count; newest first
        s_recv2 = receive(owner, filt, 1.0, wo2)
        _, everything = listing(owner)
        _, unread = listing(owner, "&unread=true")
        c_before = count(owner)
        record("MN-003", s_recv2 == 200 and [i["partName"] for i in everything] == ["qa-ga-mn-filter", "qa-ga-mn-pads"]
               and len(unread) == 2 and c_before == 2 and count(mech) == 2,
               f"two notices, newest first {[i['partName'] for i in everything]}; ?unread=true returns {len(unread)}; the counter reads {c_before} (the mechanic's {count(mech)})")

        # MN-004 -- mark one as read: idempotent, counter drops, filter follows
        target = everything[1]
        s1, r1 = http("PUT", f"{BASE}/uuid/{target['uuid']}/read", token=mech)
        s2, r2 = http("PUT", f"{BASE}/uuid/{target['uuid']}/read", token=owner)
        _, unread_after = listing(owner, "&unread=true")
        _, all_after = listing(owner)
        record("MN-004", s1 == 200 and r1["read"] is True and r1.get("readAt") and s2 == 200 and r2["readAt"] == r1["readAt"]
               and count(owner) == 1 and [i["partName"] for i in unread_after] == ["qa-ga-mn-filter"] and len(all_after) == 2,
               f"read by the mechanic {s1} (readAt set), again by the owner {s2} with the same readAt; counter {count(owner)}; unread filter {[i['partName'] for i in unread_after]}; the inbox still lists {len(all_after)}")

        # MN-005 -- mark all as read
        receive(owner, part(owner, "mn-extra"), 3.0, wo2)
        c_pending = count(owner)
        s_all, body = http("PUT", f"{BASE}/read-all", token=mech)
        s_again, body2 = http("PUT", f"{BASE}/read-all", token=mech)
        record("MN-005", c_pending == 2 and s_all == 200 and body == {"marked": 2} and count(owner) == 0 and s_again == 200 and body2 == {"marked": 0}
               and listing(owner, "&unread=true")[1] == [],
               f"{c_pending} unread -> read-all {s_all} {body}; counter {count(owner)}; a second call {s_again} {body2}; the unread filter is empty")
        # MN-006 -- scope: other tenant, and the roles
        owner_b, _ = da.second_tenant(admin)
        unknown = http("PUT", f"{BASE}/uuid/{NIL}/read", token=owner_b)
        cross = http("PUT", f"{BASE}/uuid/{target['uuid']}/read", token=owner_b)
        b_view = (listing(owner_b)[1], count(owner_b), http("PUT", f"{BASE}/read-all", token=owner_b))
        refused = {name: (http("GET", BASE, token=t)[0], http("GET", f"{BASE}/unread-count", token=t)[0],
                          http("PUT", f"{BASE}/uuid/{target['uuid']}/read", token=t)[0], http("PUT", f"{BASE}/read-all", token=t)[0])
                   for name, t in (("TenantUser", member), ("Driver", driver))}
        allowed = {name: (http("GET", BASE, token=t)[0], http("GET", f"{BASE}/unread-count", token=t)[0], http("PUT", f"{BASE}/read-all", token=t)[0])
                   for name, t in (("TenantOwner", owner), ("Mechanic", mech))}
        anonymous, neighbour = http("GET", BASE)[0], http("GET", "/purchase-order")[0]   # the API answers a missing token the same way everywhere
        record("MN-006", cross[0] == 404 and cross[1] == unknown[1] and b_view[0] == [] and b_view[1] == 0 and b_view[2][1] == {"marked": 0}
               and all(v == (403,) * 4 for v in refused.values()) and all(v == (200,) * 3 for v in allowed.values()) and anonymous == neighbour and anonymous in (401, 403),
               f"owner B reading A's notice {cross[0]} (the body equals an unknown uuid's), sees {len(b_view[0])} notices, counter {b_view[1]}, read-all marks {b_view[2][1]}; "
               f"tenant user and driver {refused}; owner and mechanic {allowed}; no token {anonymous}")
        if keep:   # leave the walkthrough's raw material: unread notices for the console
            receive(owner, part(owner, "mn-ui-1"), 5.0, wo1)
            receive(owner, part(owner, "mn-ui-2"), 6.0, wo2)
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
