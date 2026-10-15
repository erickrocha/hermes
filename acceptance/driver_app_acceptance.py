#!/usr/bin/env python3
"""Acceptance for the backend half of C-031 / KAN-34 (driver app screens): distinct checklist refusals,
the odometer guard and the tenant's default checklist template. Scenarios DA-001...DA-004.

Reuses garage_monitor_acceptance.py's helpers and `qa-ga-` fixtures (API + dev DB), like driver_scope_acceptance.py.
"""
import json, os, sys, urllib.error, urllib.request
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import garage_monitor_acceptance as b  # noqa: E402
import driver_scope_acceptance as d  # noqa: E402
import role_matrix_audit as audit  # noqa: E402

http, record = b.http, b.record
NIL = "00000000-0000-4000-8000-000000000000"


def localized(method, path, body, token, lang):
    """Like b.http but sends Accept-Language and returns the whole error body."""
    request = urllib.request.Request(b.BASE + path, data=json.dumps(body).encode() if body is not None else None, method=method,
                                     headers={"Content-Type": "application/json", "Authorization": f"Bearer {token}", "Accept-Language": lang})
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            return response.status, json.loads(response.read() or "null")
    except urllib.error.HTTPError as error:
        return error.code, json.loads(error.read() or "null")


def template(owner, name, kind, n_items=2, active=True):
    st, t = http("POST", "/checklist-template", {"name": name, "checklistType": kind, "active": active,
                                                 "items": [{"description": f"item {i}", "generatesWorkOrder": False} for i in range(n_items)]}, owner)
    assert st == 201, (name, st, t)
    t = http("GET", f"/checklist-template/uuid/{t['uuid']}", token=owner)[1]
    return t


def run_body(tmpl, driver, vehicle, kind, km, opening=None, answers=None):
    body = {"checklistTemplateUuid": tmpl["uuid"], "driverUuid": driver["uuid"], "vehicleUuid": vehicle["uuid"],
            "checklistType": kind, "odometerKm": km,
            "answers": answers if answers is not None else [{"checklistTemplateItemUuid": tmpl["items"][0]["uuid"], "status": "Conforming"}]}
    if opening:
        body["openingChecklistUuid"] = opening
    return body


def cleanup():
    # A Return points at its Departure; the FK-blind generic cleanup cannot delete such pairs in one statement.
    b.sql("UPDATE checklist_run SET opening_checklist_id=NULL WHERE tenant_id IN (SELECT id FROM tenant WHERE business_name LIKE 'qa-ga-%')", check=False)
    audit.generic_cleanup()


def run():
    cleanup()
    try:
        admin, owner = b.seed()
        da, da_t = d.make_user(owner, "qa-ga-da-a@hermes.test", "Driver")
        db_, db_t = d.make_user(owner, "qa-ga-da-b@hermes.test", "Driver")
        v1 = d.vehicle(owner, "DA1"); v2 = d.vehicle(owner, "DA2")
        for drv, veh in ((da, v1), (da, v2), (db_, d.vehicle(owner, "DA3"))):
            assert http("POST", f"/vehicle/uuid/{veh['uuid']}/assignment", {"driverUuid": drv["uuid"]}, owner)[0] == 201
        dep = template(owner, "qa-ga-dep", "Departure")
        ret = template(owner, "qa-ga-ret", "Return")

        # DA-001 -- every refusal carries its own key (SR-CA-03-07) and a Portuguese text
        st, dep1 = http("POST", "/checklist-run", run_body(dep, da, v1, "Departure", 1000.0), da_t)
        assert st == 201, (st, dep1)
        cases = {}
        def refuse(label, expected_status, expected_key, body, token=da_t):
            s, p = http("POST", "/checklist-run", body, token)
            cases[label] = (s, b.ek(p))
            return s == expected_status and b.ek(p) == expected_key
        ok = [
            refuse("second departure", 409, "DriverAlreadyHoldsAVehicle", run_body(dep, da, v2, "Departure", 1100.0)),
            refuse("return without opening", 400, "ReturnRequiresOpeningChecklist", run_body(ret, da, v1, "Return", 1100.0)),
            refuse("departure naming an opening", 400, "MayNotNameAnOpeningChecklist", run_body(dep, da, v1, "Departure", 1100.0, opening=dep1["uuid"])),
            refuse("opening that does not exist", 400, "OpeningChecklistNotFound", run_body(ret, da, v1, "Return", 1100.0, opening=NIL)),
            refuse("opening of another vehicle", 400, "OpeningChecklistWrongVehicle", run_body(ret, da, v2, "Return", 1100.0, opening=dep1["uuid"])),
            refuse("no answers", 400, "AtLeastOneAnswerRequired", run_body(dep, da, v2, "Standalone", 1100.0, answers=[])),
            refuse("duplicate answer", 400, "DuplicateAnswerForItem", run_body(dep, da, v2, "Standalone", 1100.0, answers=[
                {"checklistTemplateItemUuid": dep["items"][0]["uuid"], "status": "Conforming"}, {"checklistTemplateItemUuid": dep["items"][0]["uuid"], "status": "Conforming"}])),
            refuse("item of another template", 400, "AnswerNotATemplateItem", run_body(dep, da, v2, "Standalone", 1100.0, answers=[
                {"checklistTemplateItemUuid": ret["items"][0]["uuid"], "status": "Conforming"}])),
        ]
        s_close, close = http("POST", "/checklist-run", run_body(ret, da, v1, "Return", 1100.0, opening=dep1["uuid"]), da_t)
        ok.append(s_close == 201)
        ok.append(refuse("opening already closed", 400, "OpeningChecklistAlreadyClosed", run_body(ret, da, v1, "Return", 1200.0, opening=dep1["uuid"])))
        keys = [k for _, k in cases.values()]
        record("DA-001", all(ok) and len(set(keys)) == len(keys),
               "; ".join(f"{label} {s} {k}" for label, (s, k) in cases.items()))

        # DA-002 -- odometer guard (SR-CA-03-08): official reading is now 1100
        below = http("POST", "/checklist-run", run_body(dep, da, v1, "Standalone", 1099.0), da_t)
        negative = http("POST", "/checklist-run", run_body(dep, da, v1, "Standalone", -1.0), da_t)
        equal = http("POST", "/checklist-run", run_body(dep, da, v1, "Standalone", 1100.0), da_t)
        official = http("GET", f"/vehicle/uuid/{v1['uuid']}", token=owner)[1].get("odometerKm")
        s_fix, _ = http("POST", f"/vehicle/uuid/{v1['uuid']}/km-evolution", {"km": 1050.0, "origin": "Manual", "recordedAt": b.datetime.utcnow().isoformat(timespec="seconds")}, owner)
        corrected = http("GET", f"/vehicle/uuid/{v1['uuid']}", token=owner)[1].get("odometerKm")
        after_fix = http("POST", "/checklist-run", run_body(dep, da, v1, "Standalone", 1050.0), da_t)[0]
        record("DA-002", (below[0], b.ek(below[1])) == (400, "OdometerBelowOfficial") and (negative[0], b.ek(negative[1])) == (400, "OdometerNegative")
               and equal[0] == 201 and official == 1100.0 and s_fix == 201 and corrected == 1050.0 and after_fix == 201,
               f"1099 below 1100 -> {below[0]} {b.ek(below[1])}; -1 -> {negative[0]} {b.ek(negative[1])}; 1100 equal -> {equal[0]}; official {official}; "
               f"manual correction to 1050 -> {s_fix}, official {corrected}, a checklist at 1050 -> {after_fix}")

        # DA-003 -- the default template (SR-CA-03-09)
        dep2 = template(owner, "qa-ga-dep2", "Departure")
        stand = template(owner, "qa-ga-stand", "Standalone")
        inactive = template(owner, "qa-ga-off", "Departure", active=False)
        put = lambda t, token: http("PUT", f"/checklist-template/uuid/{t['uuid']}/default", None, token)
        s1, r1 = put(dep, owner)
        s2, r2 = put(dep2, owner)
        listing = {t["name"]: t["isDefault"] for t in http("GET", "/checklist-template?page=0&pageSize=50", token=da_t)[1]["items"]}
        s_again, _ = put(dep2, owner)
        s_stand, k_stand = put(stand, owner)
        s_off, k_off = put(inactive, owner)
        s_drv, _ = put(dep2, da_t)
        s_other, _ = put({"uuid": NIL}, owner)
        record("DA-003", s1 == 200 and r1["isDefault"] and s2 == 200 and r2["isDefault"]
               and listing.get("qa-ga-dep2") is True and listing.get("qa-ga-dep") is False and s_again == 200
               and (s_stand, b.ek(k_stand)) == (400, "ChecklistTemplateCannotBeDefault") and (s_off, b.ek(k_off)) == (400, "ChecklistTemplateCannotBeDefault")
               and s_drv == 403 and s_other == 404,
               f"owner marks dep {s1}, then dep2 {s2}; the driver's list shows dep2 default={listing.get('qa-ga-dep2')}, dep={listing.get('qa-ga-dep')}; again {s_again}; "
               f"Standalone {s_stand}, inactive {s_off}; a driver {s_drv}; an unknown uuid {s_other}")

        # DA-004 -- localization: the same refusal in pt-BR, es and en reads differently and is not the generic text
        texts = {}
        for lang in ("pt-BR", "es", "en"):
            s, p = localized("POST", "/checklist-run", run_body(dep, da, v1, "Standalone", 1.0), da_t, lang)
            texts[lang] = (s, p.get("message") if isinstance(p, dict) else p)
        record("DA-004", all(s == 400 for s, _ in texts.values()) and len({m for _, m in texts.values()}) == 3
               and "quilometragem" in texts["pt-BR"][1].lower() and "kilometraje" in texts["es"][1].lower(),
               f"pt-BR: {texts['pt-BR'][1]!r}; es: {texts['es'][1]!r}; en: {texts['en'][1]!r}")
    finally:
        cleanup()
    bad = [k for k, v in b.RESULTS.items() if v["status"] != "PASS"]
    print(f"{len(b.RESULTS) - len(bad)}/{len(b.RESULTS)} scenarios PASS")
    return 1 if bad else 0


if __name__ == "__main__":
    try:
        sys.exit(run())
    except Exception as error:
        print(f"QA BLOCKED: {type(error).__name__}: {error}", file=sys.stderr)
        try:
            cleanup()
        except Exception as e2:
            print(f"cleanup: {e2}", file=sys.stderr)
        sys.exit(2)
