#!/usr/bin/env python3
"""Billing acceptance suite -- C-015 / Gate 3.

Runs against a live Hermes API and a migrated MariaDB. Test rows use a unique
prefix and are deleted on exit; unrelated tenant data is never removed.
"""
import argparse
import os
import re
import sys
import time
import uuid
from datetime import datetime

import tenancy_plans_acceptance as tp


RESULTS = {}


def check(scenario, passed, evidence):
    status = "PASS" if passed else "FAIL"
    RESULTS[scenario] = status
    print(f"{scenario} {status}: {evidence}", flush=True)


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def create_user(fx, creator_token, email, role, tenant_id=None):
    body = {"email": email, "name": email.split("@")[0], "role": role, "enabled": True}
    if tenant_id is not None:
        body["tenantId"] = tenant_id
    status, payload = tp.http("POST", "/user", body=body, token=creator_token)
    require(status == 201, f"POST /user {role} -> {status} {payload}")
    return tp.activate(fx, email)


def page_for(token):
    return tp.http("GET", "/billing/charges?page=0&pageSize=100", token=token)


def wait_for_charges(fx, tenant_uuids, timeout=30):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        status, payload = page_for(fx.adm)
        if status == 200:
            found = {charge["tenantUuid"] for charge in payload["items"]}
            if set(tenant_uuids).issubset(found):
                return {charge["tenantUuid"]: charge for charge in payload["items"]
                        if charge["tenantUuid"] in tenant_uuids}
        time.sleep(0.25)
    raise RuntimeError(f"billing job did not close cycles for tenants {tenant_uuids}")


def run(fx):
    admin = fx.adm
    owner_a = fx.ownerA["token"]
    owner_b = fx.ownerB["token"]
    tenant_a = fx.A
    tenant_b = fx.B
    uuid_a = tenant_a["uuid"]
    uuid_b = tenant_b["uuid"]

    fx.driver_a = create_user(fx, owner_a, f"{tp.TAG}-driver-a@hermes.test", "Driver")
    fx.mechanic_a = create_user(fx, owner_a, f"{tp.TAG}-mechanic-a@hermes.test", "Mechanic")

    plan_body = tp.plan_body(f"{tp.TAG}-billing-plan", price=1999, users=1, days=30, overage=250)
    status_plan, plan = tp.create_plan(admin, plan_body)
    status_next, next_plan = tp.create_plan(
        admin, tp.plan_body(f"{tp.TAG}-billing-next", price=2999, users=2, days=45, overage=400)
    )
    payment_date_columns = tp.one(
        "SELECT COUNT(*) FROM information_schema.columns WHERE table_schema='hermes' "
        "AND table_name='business_plan' AND column_name='payment_date'"
    )
    openapi_status, openapi = tp.http("GET", "/api-docs/openapi.json")
    documented_paths = set(openapi.get("paths", {})) if isinstance(openapi, dict) else set()
    schemas = openapi.get("components", {}).get("schemas", {}) if isinstance(openapi, dict) else {}
    plan_schema_names = ("BusinessPlanJson", "CreateBusinessPlanJson", "UpdateBusinessPlanJson")
    no_payment_date_schema = all(
        name in schemas and "paymentDate" not in schemas[name].get("properties", {})
        for name in plan_schema_names
    )
    required_paths = {
        "/billing/charges",
        "/billing/charges/uuid/{uuid}/confirm",
        "/billing/tenant/uuid/{uuid}/settings",
        "/billing/tenant/uuid/{uuid}/suspension",
    }
    no_external_billing_paths = not any(
        any(term in path.lower() for term in ("gateway", "invoice", "fiscal"))
        for path in documented_paths
    )
    check("TP-082", status_plan == 201 and status_next == 201 and
          plan.get("pricePerExtraUserInCents") == 250 and "paymentDate" not in plan and
          payment_date_columns == "0" and openapi_status == 200 and
          required_paths.issubset(documented_paths) and no_external_billing_paths and
          no_payment_date_schema,
          f"overage=250; billing routes documented; no gateway/invoice paths; "
          f"paymentDate absent from API/schema ({payment_date_columns} columns)")
    require(status_plan == 201 and status_next == 201, "could not create billing test plans")

    assigned_a, _ = tp.http(
        "POST", f"/tenant/uuid/{uuid_a}/plan", body={"businessPlanId": plan["id"]}, token=admin
    )
    assigned_b, _ = tp.http(
        "POST", f"/tenant/uuid/{uuid_b}/plan", body={"businessPlanId": plan["id"]}, token=admin
    )
    configured_a, _ = tp.http(
        "PUT", f"/billing/tenant/uuid/{uuid_a}/settings", body={"graceDays": 3}, token=admin
    )
    configured_b, _ = tp.http(
        "PUT", f"/billing/tenant/uuid/{uuid_b}/settings", body={"graceDays": 60}, token=admin
    )
    initial_a, initial_settings_a = tp.http(
        "GET", f"/billing/tenant/uuid/{uuid_a}/settings", token=admin
    )
    change_status, _ = tp.http(
        "POST", f"/tenant/uuid/{uuid_a}/plan", body={"businessPlanId": next_plan["id"]}, token=admin
    )
    pending_status, pending_settings = tp.http(
        "GET", f"/billing/tenant/uuid/{uuid_a}/settings", token=admin
    )
    current_status, current_plan = tp.http("GET", f"/tenant/uuid/{uuid_a}/plan", token=admin)
    check("TP-083", configured_a == 200 and configured_b == 200 and assigned_a == 200 and
          assigned_b == 200 and initial_a == 200 and initial_settings_a.get("cycleStartedAt") and
          change_status == 200 and pending_status == 200 and pending_settings.get("pendingPlan") and
          current_status == 200 and current_plan.get("id") == plan["id"],
          "first assignment started cycle; replacement plan remains pending until close")

    tp.sql(
        "UPDATE tenant_billing SET cycle_started_at=DATE_SUB(UTC_TIMESTAMP(), INTERVAL 40 DAY) "
        f"WHERE tenant_id IN ({tenant_a['id']},{tenant_b['id']})"
    )
    charges = wait_for_charges(fx, {uuid_a, uuid_b})
    charge_a = charges[uuid_a]
    charge_b = charges[uuid_b]
    expected_amount = 1999 + 3 * 250
    plan_after, applied_plan = tp.http("GET", f"/tenant/uuid/{uuid_a}/plan", token=admin)
    settings_after, closed_settings = tp.http(
        "GET", f"/billing/tenant/uuid/{uuid_a}/settings", token=admin
    )
    check("TP-084", charge_a["status"] == "Pending" and charge_a["currency"] == "BRL" and
          charge_a["planName"] == plan["name"] and charge_a["priceInCents"] == 1999 and
          charge_a["pricePerExtraUserInCents"] == 250 and charge_a["includedUsers"] == 1 and
          charge_a["activeUsers"] == 4 and charge_a["extraUsers"] == 3 and
          charge_a["totalInCents"] == expected_amount and charge_a["graceDays"] == 3 and
          charge_a["isOverdue"] and charge_b["totalInCents"] == 1999 and
          charge_b["graceDays"] == 60 and not charge_b["isOverdue"] and
          plan_after == 200 and applied_plan.get("id") == next_plan["id"] and
          settings_after == 200 and not closed_settings.get("pendingPlan"),
          f"A={charge_a['activeUsers']} users; total={charge_a['totalInCents']} cents; "
          f"B isolated; pending plan applied={applied_plan.get('id')}")

    owner_page_status, owner_page = page_for(owner_a)
    owner_b_status, owner_b_page = page_for(owner_b)
    admin_page_status, admin_page = page_for(admin)
    owner_uuids = {item["tenantUuid"] for item in owner_page.get("items", [])} if owner_page_status == 200 else set()
    owner_b_uuids = {item["tenantUuid"] for item in owner_b_page.get("items", [])} if owner_b_status == 200 else set()
    admin_uuids = {item["tenantUuid"] for item in admin_page.get("items", [])} if admin_page_status == 200 else set()
    owner_a_charge = next((item for item in owner_page.get("items", [])
                           if item["tenantUuid"] == uuid_a), {}) if owner_page_status == 200 else {}
    owner_b_charge = next((item for item in owner_b_page.get("items", [])
                           if item["tenantUuid"] == uuid_b), {}) if owner_b_status == 200 else {}
    check("TP-087", owner_page_status == 200 and owner_uuids == {uuid_a} and
          owner_a_charge.get("isOverdue") and owner_b_status == 200 and owner_b_uuids == {uuid_b} and
          not owner_b_charge.get("isOverdue") and admin_page_status == 200 and
          {uuid_a, uuid_b}.issubset(admin_uuids),
          "Owners see only their own history; A is overdue, B remains inside its 60-day grace period")
    check("TP-088", admin_page_status == 200 and {uuid_a, uuid_b}.issubset(admin_uuids) and
          owner_uuids == {uuid_a}, "two tenant ledgers are isolated by caller scope")

    tenant_a_id = tenant_a["id"]
    tenant_b_id = tenant_b["id"]
    tp.sql("UPDATE tenant_billing SET cycle_started_at=UTC_TIMESTAMP() "
           f"WHERE tenant_id IN ({tenant_a_id},{tenant_b_id})")
    time.sleep(1.5)
    charge_count_a = tp.one(f"SELECT COUNT(*) FROM billing_charge WHERE tenant_id={tenant_a_id}")
    charge_count_b = tp.one(f"SELECT COUNT(*) FROM billing_charge WHERE tenant_id={tenant_b_id}")
    check("TP-089", charge_count_a == "1" and charge_count_b == "1",
          f"repeat job created no duplicate ledger rows (A={charge_count_a}, B={charge_count_b})")

    charge_uuid = charge_a["uuid"]
    owner_confirm, _ = tp.http("POST", f"/billing/charges/uuid/{charge_uuid}/confirm", token=owner_a)
    owner_configure, _ = tp.http(
        "PUT", f"/billing/tenant/uuid/{uuid_a}/settings", body={"graceDays": 7}, token=owner_a
    )
    check("TP-086", owner_confirm == 403 and owner_configure == 403,
          f"TenantOwner cannot confirm ({owner_confirm}) or configure ({owner_configure})")

    first_confirm_status, first_confirm = tp.http(
        "POST", f"/billing/charges/uuid/{charge_uuid}/confirm", token=admin
    )
    repeat_status, repeated = tp.http(
        "POST", f"/billing/charges/uuid/{charge_uuid}/confirm", token=admin
    )
    charge_count_after_confirm = tp.one(f"SELECT COUNT(*) FROM billing_charge WHERE tenant_id={tenant_a_id}")
    first_paid_at = datetime.fromisoformat(first_confirm["paidAt"]).replace(microsecond=0) \
        if first_confirm.get("paidAt") else None
    repeat_paid_at = datetime.fromisoformat(repeated["paidAt"]).replace(microsecond=0) \
        if repeated.get("paidAt") else None
    check("TP-085", first_confirm_status == 200 and first_confirm.get("status") == "Paid" and
          first_confirm.get("paidAt") and first_confirm.get("paidBy") and repeat_status == 200 and
          first_paid_at is not None and repeat_paid_at == first_paid_at and charge_count_after_confirm == "1",
            f"first={first_confirm_status}/{first_confirm}; repeat={repeat_status}/{repeated}; "
            f"ledgerRows={charge_count_after_confirm}")

    suspension_status, suspended = tp.http(
        "PUT", f"/billing/tenant/uuid/{uuid_a}/suspension", body={"suspended": True}, token=admin
    )
    blocked = {}
    for role, actor in (("TenantOwner", fx.ownerA), ("TenantUser", fx.userA),
                        ("Driver", fx.driver_a), ("Mechanic", fx.mechanic_a)):
        blocked[role], _ = tp.http("GET", f"/tenant/uuid/{uuid_a}", token=actor["token"])
    anonymous, _ = tp.http("GET", f"/tenant/uuid/{uuid_a}")
    check("TP-090", suspension_status == 200 and suspended.get("suspendedAt") and
          all(status == 403 for status in blocked.values()) and anonymous in (401, 403),
          f"suspension blocks every tenant role {blocked}; anonymous remains unauthenticated ({anonymous})")

    restore_status, restored = tp.http(
        "PUT", f"/billing/tenant/uuid/{uuid_a}/suspension", body={"suspended": False}, token=admin
    )
    restored_reads = {role: tp.http("GET", f"/tenant/uuid/{uuid_a}", token=actor["token"])[0]
                      for role, actor in (("TenantOwner", fx.ownerA), ("TenantUser", fx.userA),
                                          ("Driver", fx.driver_a), ("Mechanic", fx.mechanic_a))}
    check("TP-091", restore_status == 200 and restored.get("suspendedAt") is None and
          all(status == 200 for status in restored_reads.values()),
          f"restoration reopens access for all roles {restored_reads}")

    invoice_keys = {key for key in charge_a if "invoice" in key.lower() or "fiscal" in key.lower()}
    owner_history_status, owner_history = page_for(owner_a)
    owner_charge = next((item for item in owner_history.get("items", [])
                         if item["tenantUuid"] == uuid_a), {}) if owner_history_status == 200 else {}
    check("TP-092", owner_history_status == 200 and owner_charge.get("currency") == "BRL" and
          owner_charge.get("planName") == plan["name"] and not invoice_keys,
          "Owner history exposes BRL charge snapshots; no invoice/fiscal fields are emitted")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--keep", action="store_true", help="retain fixtures for browser testing")
    parser.add_argument("--cleanup-tag", help="remove fixtures retained by a previous run")
    args = parser.parse_args()
    tp.BASE = os.environ.get("HERMES_API", "http://127.0.0.1:8081")
    tp.ADMIN_EMAIL = os.environ.get("HERMES_ADMIN_EMAIL", "admin@hermes.dev")
    tp.ADMIN_PASSWORD = os.environ.get("HERMES_ADMIN_PASSWORD", "LocalDevOnly123!")
    tp.DB_CONTAINER = os.environ.get("HERMES_DB_CONTAINER", "dev-mariadb-1")
    tp.DB_PASS = os.environ.get("HERMES_DB_PASSWORD", "brutal")
    if args.cleanup_tag:
        if not re.fullmatch(r"qa-bill-[a-f0-9]{10}", args.cleanup_tag):
            parser.error("--cleanup-tag must be a qa-bill-[10 hex digits] prefix")
        tp.TAG = args.cleanup_tag
        tenant_ids = [row[0] for row in tp.sql(
            f"SELECT id FROM tenant WHERE business_name LIKE '{tp.TAG}-%'"
        )]
        if tenant_ids:
            ids = ",".join(tenant_ids)
            tp.sql(f"DELETE FROM billing_charge WHERE tenant_id IN ({ids})")
            tp.sql(f"DELETE FROM tenant_billing WHERE tenant_id IN ({ids})")
        tp.cleanup()
        print(f"Cleanup complete for {tp.TAG}.", flush=True)
        return 0

    tp.TAG = f"qa-bill-{uuid.uuid4().hex[:10]}"
    tp.CREATED_TENANTS.clear()
    tp.CREATED_PLANS.clear()
    fx = tp.Fx()
    fx.A = None
    fx.B = None

    if tp.http("GET", "/api-docs/openapi.json")[0] != 200:
        print(f"API is not reachable at {tp.BASE}", file=sys.stderr)
        return 2

    try:
        fx.secret = os.environ.get("ACCESS_TOKEN_SECRET") or tp.dotenv().get("ACCESS_TOKEN_SECRET")
        require(fx.secret, "ACCESS_TOKEN_SECRET must be supplied to the test process")
        fx.adm = tp.token_for(tp.ADMIN_EMAIL, tp.ADMIN_PASSWORD)
        for label, tax_id in (("A", f"QAB{uuid.uuid4().hex[:11]}"),
                              ("B", f"QAB{uuid.uuid4().hex[:11]}")):
            status, tenant = tp.create_tenant(
                fx.adm,
                {"businessName": f"{tp.TAG}-tenant-{label}", "taxId": tax_id, "countryCode": "US"},
            )
            require(status == 201, f"create tenant {label} -> {status} {tenant}")
            setattr(fx, label, tenant)
        fx.ownerA = create_user(fx, fx.adm, f"{tp.TAG}-owner-a@hermes.test", "TenantOwner", fx.A["id"])
        fx.ownerB = create_user(fx, fx.adm, f"{tp.TAG}-owner-b@hermes.test", "TenantOwner", fx.B["id"])
        fx.userA = create_user(fx, fx.ownerA["token"], f"{tp.TAG}-user-a@hermes.test", "TenantUser")
        run(fx)
    except Exception as error:
        print(f"Billing acceptance setup/runtime error: {error}", file=sys.stderr)
        RESULTS["SETUP"] = "FAIL"
    finally:
        if args.keep:
            print(f"Fixtures retained: tag={tp.TAG}; ownerA={getattr(fx, 'ownerA', {}).get('email')}; "
                  f"ownerB={getattr(fx, 'ownerB', {}).get('email')}; "
                  f"password={tp.FIXTURE_PW}", flush=True)
        else:
            if fx.A is not None and fx.B is not None:
                tenant_ids = f"{fx.A['id']},{fx.B['id']}"
                tp.sql(f"DELETE FROM billing_charge WHERE tenant_id IN ({tenant_ids})")
                tp.sql(f"DELETE FROM tenant_billing WHERE tenant_id IN ({tenant_ids})")
            tp.cleanup()
            print(f"Cleanup complete for {tp.TAG}.", flush=True)

    failed = [scenario for scenario, result in RESULTS.items() if result == "FAIL"]
    print(f"\nBilling acceptance: {len(RESULTS) - len(failed)}/{len(RESULTS)} passed; "
          f"failed={failed}", flush=True)
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())