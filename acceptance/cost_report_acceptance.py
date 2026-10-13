#!/usr/bin/env python3
"""Gate 3 acceptance for C-030 EPIC-SP-07/SP-08 (HRMS-804...810): vehicle cost and result, and the two cost reports. AC-070...AC-076.

Reuses garage_monitor_acceptance.py's helpers and `qa-ga-` fixtures. Work-order postings, dates and the external-service
fields are set by SQL (there is no API that back-dates a work order); everything else goes through the API.
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import garage_monitor_acceptance as b  # noqa: E402
from access_profile_acceptance import activate  # noqa: E402

http, sql, one, record = b.http, b.sql, b.one, b.record
FROM, TO = "2026-09-01", "2026-09-30"
OCT_FROM, OCT_TO = "2026-10-01", "2026-10-31"


def raw(method, path, token):
    """The body as text, with the headers (the CSV endpoints are not JSON)."""
    import urllib.request, urllib.error
    req = urllib.request.Request(b.BASE + path, method=method, headers={"Authorization": f"Bearer {token}"})
    try:
        with urllib.request.urlopen(req, timeout=20) as r:
            return r.status, {k.lower(): v for k, v in r.headers.items()}, r.read().decode("utf-8")
    except urllib.error.HTTPError as e:
        return e.code, {k.lower(): v for k, v in e.headers.items()}, e.read().decode("utf-8")


def hexid(uuid):
    return "UNHEX('" + uuid.replace("-", "") + "')"


def cleanup_extra():
    ids = ",".join(r[0] for r in b.rows(f"SELECT id FROM tenant WHERE business_name LIKE '{b.TAG}-%'")) or "0"
    for t in ("work_order_posting", "work_order_item", "work_order_pendency", "work_order", "part", "vehicle_expense", "extra_trip"):
        sql(f"DELETE FROM {t} WHERE tenant_id IN ({ids})", check=False)


def run():
    cleanup_extra(); b.cleanup()
    try:
        admin, owner = b.seed()
        tid = b.TENANT_ID
        tu, tu_t = activate(owner, "TenantUser")
        dr, dr_t = activate(owner, "Driver")
        me, me_t = activate(owner, "Mechanic")
        v1, v2 = b.create_vehicle(owner, "C1"), b.create_vehicle(owner, "C2")

        part = http("POST", "/part", {"name": "QA pad", "unit": "un"}, owner)[1]
        def work_order(desc, km):
            s, w = http("POST", "/work-order", {"vehicleUuid": v1["uuid"], "odometerKm": km, "description": desc}, owner)
            assert s == 201, (s, w)
            return w
        w1, w2, w3, w4 = work_order("Brakes", 100), work_order("Gearbox", 110), work_order("Tyres", 120), work_order("Cancelled one", 130)
        def posting(w, cents):
            wid = one(f"SELECT id FROM work_order WHERE uuid={hexid(w['uuid'])}")
            sql(f"INSERT INTO work_order_posting (uuid, tenant_id, work_order_id, part_id, quantity, unit_value_cents, total_value_cents, cost_source, created_at, updated_at) "
                f"VALUES (UNHEX(REPLACE(UUID(),'-','')), {tid}, {wid}, {one(f"SELECT id FROM part WHERE uuid={hexid(part['uuid'])}")}, 1, {cents}, {cents}, 'Manual', NOW(), NOW())")
        for cents in (10000, 2500):
            posting(w1, cents)
        posting(w4, 99999)
        sql(f"UPDATE work_order SET service_type='Brakes', opened_at='2026-09-10 12:00:00' WHERE uuid={hexid(w1['uuid'])}")
        sql(f"UPDATE work_order SET external_service=1, supplier='Oficina Sul', invoice_number='NF-2', invoice_value_cents=30000, service_type='Gearbox', opened_at='2026-09-12 10:00:00', concluded_at='2026-10-02' WHERE uuid={hexid(w2['uuid'])}")
        sql(f"UPDATE work_order SET external_service=1, supplier='oficina sul ', invoice_number='NF-3', invoice_value_cents=45000, service_type=NULL, opened_at='2026-09-15 10:00:00', concluded_at='2026-09-20' WHERE uuid={hexid(w3['uuid'])}")
        sql(f"UPDATE work_order SET status='Cancelled', external_service=1, supplier='Oficina Sul', invoice_value_cents=88888, opened_at='2026-09-11 10:00:00' WHERE uuid={hexid(w4['uuid'])}")

        assert http("POST", "/fuel-entry", {"vehicleUuid": v1["uuid"], "recordedAt": "2026-09-15T10:00:00", "volumeLiters": 50.0, "valueCents": 10000, "odometerKm": 500.0, "fullTank": True, "station": "QA"}, owner)[0] == 201
        assert http("POST", "/vehicle-expense", {"vehicleUuid": v1["uuid"], "category": "Toll", "competencePeriod": "2026-09", "issueDate": "2026-09-16", "valueCents": 7000}, owner)[0] == 201
        assert http("POST", "/vehicle-expense", {"vehicleUuid": v1["uuid"], "category": "Toll", "competencePeriod": "2026-08", "issueDate": "2026-08-30", "valueCents": 555}, owner)[0] == 201
        assert http("POST", "/extra-trip", {"orderCode": "QA-T1", "tripDate": "2026-09-20", "status": "Scheduled", "vehicleUuid": v1["uuid"], "freightValueCents": 200000}, owner)[0] == 201
        s_c, tc = http("POST", "/extra-trip", {"orderCode": "QA-T2", "tripDate": "2026-09-21", "status": "Scheduled", "vehicleUuid": v1["uuid"], "freightValueCents": 50000}, owner)
        sql(f"UPDATE extra_trip SET status='Cancelled' WHERE uuid={hexid(tc['uuid'])}")
        # a service performed today, on V1's triage
        svc = http("POST", "/garage-service", {"tenantId": tid, "name": "QA Wash", "displayOrder": 0, "active": True, "serviceGroup": "External", "requiredForDeparture": False, "governedByTank": False, "applicability": {"scope": "all"}}, owner)
        att = b.open_attendance(owner, v1)
        line = [s for s in att["services"] if s["name"] == "QA Wash"][0]
        assert http("PUT", f"/garage-attendance/uuid/{att['uuid']}/service/{line['uuid']}", {"state": "Performed"}, owner)[0] == 200

        cost = lambda v, f, t, tok=tu_t: http("GET", f"/cost-report/vehicle/uuid/{v['uuid']}?from={f}&to={t}", token=tok)

        # AC-070 -- the four sources, dated by the work order's conclusion else opening; cancelled and out-of-month left out
        s, c = cost(v1, FROM, TO)
        so = c.get("bySource", {})
        so_oct = cost(v1, OCT_FROM, OCT_TO)[1].get("bySource", {})
        record("AC-070", s == 200 and so == {"workOrderPostingsCents": 12500, "externalServicesCents": 45000, "fuelCents": 10000, "expensesCents": 7000}
               and c["totalCostCents"] == 74500 and so_oct.get("externalServicesCents") == 30000 and so_oct.get("workOrderPostingsCents") == 0,
               f"September {so} total {c.get('totalCostCents')}; October external {so_oct.get('externalServicesCents')} (the Gearbox invoice, concluded 2 Oct)")

        # AC-071 -- services are quantities, never cost
        oct_c = cost(v1, OCT_FROM, OCT_TO)[1]
        record("AC-071", c["servicesPerformed"] == [] and oct_c["servicesPerformed"] == [{"name": "QA Wash", "amount": 1}] and oct_c["totalCostCents"] == 30000,
               f"September {c['servicesPerformed']}; October {oct_c['servicesPerformed']} with cost {oct_c['totalCostCents']} unchanged by the service")

        # AC-072 -- revenue from priced, non-cancelled trips; no priced trip means no result
        s2, c2 = cost(v2, FROM, TO)
        record("AC-072", c["revenueCents"] == 200000 and c["resultCents"] == 200000 - 74500 and s2 == 200 and c2["revenueCents"] == 0 and "resultCents" not in c2,
               f"V1 revenue {c['revenueCents']} result {c.get('resultCents')}; V2 revenue {c2.get('revenueCents')} result {c2.get('resultCents', 'absent')}")

        # AC-073 -- grouping with the explicit heading, and a work order's own total
        groups = {g["name"]: g["amount"] for g in c["byServiceType"]}
        wo1 = http("GET", f"/cost-report/work-order/uuid/{w1['uuid']}", token=tu_t)[1]
        wo3 = http("GET", f"/cost-report/work-order/uuid/{w3['uuid']}", token=tu_t)[1]
        record("AC-073", groups == {"Brakes": 12500, "": 45000} and wo1 == {"postingsCents": 12500, "externalServiceCents": 0, "totalCents": 12500}
               and wo3 == {"postingsCents": 0, "externalServiceCents": 45000, "totalCents": 45000},
               f"groups {groups} ('' is to be classified); WO1 {wo1}; WO3 {wo3}")

        # AC-074 -- the CSV export
        st, hd, body = raw("GET", f"/cost-report/vehicle/uuid/{v1['uuid']}/export?from={FROM}&to={TO}", tu_t)
        record("AC-074", st == 200 and hd.get("content-type", "").startswith("text/csv") and "QAGAC1_2026-09-01_2026-09-30.csv" in hd.get("content-disposition", "")
               and body.startswith("﻿") and all(t in body for t in ("Work-order postings", "External services", "Fuel", "Expenses"))
               and "Subtotal,,,125.00" in body and "Subtotal,,,450.00" in body and "Subtotal,,,70.00" in body,
               f"{st} {hd.get('content-type')} {hd.get('content-disposition')}; four blocks with subtotals 125.00 / 450.00 / 100.00 / 70.00")

        # AC-075 -- the supplier closing
        names = http("GET", "/cost-report/suppliers", token=tu_t)[1]
        sep = http("GET", f"/cost-report/supplier-closing?supplier=OFICINA%20SUL&from={FROM}&to={TO}", token=tu_t)[1]
        octo = http("GET", f"/cost-report/supplier-closing?supplier=Oficina%20Sul&from={OCT_FROM}&to={OCT_TO}", token=tu_t)[1]
        none = http("GET", f"/cost-report/supplier-closing?supplier=Nobody&from={FROM}&to={TO}", token=tu_t)[1]
        st, hd, csvb = raw("GET", f"/cost-report/supplier-closing/export?supplier=Oficina%20Sul&from={FROM}&to={TO}", tu_t)
        record("AC-075", len(names) == 1 and names[0].lower().strip() == "oficina sul"
               and [l["invoiceNumber"] for l in sep["lines"]] == ["NF-3"] and sep["totalCents"] == 45000
               and [l["invoiceNumber"] for l in octo["lines"]] == ["NF-2"] and none["lines"] == [] and none["totalCents"] == 0
               and st == 200 and "NF-3" in csvb and "Fuel" not in csvb and "Toll" not in csvb and "NF-1" not in csvb,
               f"suppliers {names}; September {[l['invoiceNumber'] for l in sep['lines']]} {sep['totalCents']}; October {[l['invoiceNumber'] for l in octo['lines']]}; unknown {none['lines']}; the cancelled order's 88888 is in none of them")

        # AC-076 -- who may read, and the usual guards
        refused = {n: http("GET", f"/cost-report/vehicle/uuid/{v1['uuid']}?from={FROM}&to={TO}", token=t) for n, t in (("driver", dr_t), ("mechanic", me_t))}
        owner_ok = cost(v1, FROM, TO, owner)[0]
        bad_range = cost(v1, TO, FROM)[0]
        unknown = http("GET", f"/cost-report/vehicle/uuid/{b.NIL_UUID if hasattr(b, 'NIL_UUID') else '00000000-0000-0000-0000-000000000000'}?from={FROM}&to={TO}", token=tu_t)[0]
        no_supplier = http("GET", f"/cost-report/supplier-closing?supplier=%20&from={FROM}&to={TO}", token=tu_t)[0]
        record("AC-076", all(s == 403 and e.get("permission") == "cost-report.vehicle.read" for s, e in refused.values()) and owner_ok == 200
               and bad_range == 400 and unknown == 404 and no_supplier == 400,
               f"driver/mechanic {[(n, v[0]) for n, v in refused.items()]} naming {refused['driver'][1].get('permission')}; owner {owner_ok}; from after to {bad_range}; unknown vehicle {unknown}; blank supplier {no_supplier}")
    finally:
        if not os.environ.get("KEEP_FIXTURES"):   # the browser walkthrough keeps them, then calls this with CLEAN_ONLY
            cleanup_extra(); b.cleanup()
    bad = [k for k, v in b.RESULTS.items() if v["status"] != "PASS"]
    print(f"{len(b.RESULTS) - len(bad)}/{len(b.RESULTS)} scenarios PASS")
    return 1 if bad else 0


if __name__ == "__main__":
    if os.environ.get("CLEAN_ONLY"):
        cleanup_extra(); b.cleanup(); sys.exit(0)
    try:
        sys.exit(run())
    except Exception as error:
        import traceback; traceback.print_exc()
        print(f"QA BLOCKED: {type(error).__name__}: {error}", file=sys.stderr)
        cleanup_extra(); b.cleanup()
        sys.exit(2)
