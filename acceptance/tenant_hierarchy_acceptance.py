#!/usr/bin/env python3
"""Gate 3 acceptance for C-036 EPIC-AC-08-S02 (HRMS-873): a tenant manages its descendants, and only them.

Tree under test: root -> T (owner O) -> T1 (created by O) ; root -> B (a sibling of T). Scenarios AC-091...
"""
import os, sys, time
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import garage_monitor_acceptance as b  # noqa: E402

http, rows, one, ek, record = b.http, b.rows, b.one, b.ek, b.record
CHILD = ["tenant.child.create", "tenant.child.read", "tenant.child.update", "tenant.child.plan", "tenant.child.user.manage"]


def named(res, code):
    st, body = res
    return st == 403 and isinstance(body, dict) and body.get("permission") == code


def with_tenant(token, uuid):
    return {"X-Tenant-Uuid": uuid}


def call(method, path, token, tenant=None, body=None):
    headers = {"X-Tenant-Uuid": tenant} if tenant else None
    import urllib.request, json, urllib.error
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(b.BASE + path, data=data, method=method)
    req.add_header("Authorization", f"Bearer {token}")
    if data is not None:
        req.add_header("Content-Type", "application/json")
    if headers:
        for k, v in headers.items():
            req.add_header(k, v)
    try:
        with urllib.request.urlopen(req, timeout=20) as r:
            st, raw = r.status, r.read()
    except urllib.error.HTTPError as e:
        st, raw = e.code, e.read()
    try:
        return st, (json.loads(raw) if raw else None)
    except ValueError:
        return st, raw.decode(errors="replace")


def owner_of(admin, tenant_id, tag):
    """An activated TenantOwner of `tenant_id`, created by the platform."""
    email = f"qa-ga-{tag}@hermes.test"
    st, u = http("POST", "/user", {"email": email, "name": email, "profileUuid": b.system_profile("TenantOwner"), "enabled": True, "tenantId": tenant_id}, admin)
    assert st == 201, (tag, st, u)
    pw = rows(f"SELECT password FROM user WHERE email='{email}'")[0][0]
    assert http("POST", "/accept-invite", {"token": b.invite_token(email, pw, b.load_env()["ACCESS_TOKEN_SECRET"]), "newPassword": b.PASSWORD})[0] == 204
    return b.login(email, b.PASSWORD)


def run():
    b.cleanup()
    try:
        admin, owner = b.seed()
        t_uuid = http("GET", "/tenant", token=owner)[1]["items"][0]["uuid"]
        t_id = b.TENANT_ID
        owner_uuid = rows("SELECT LOWER(CONCAT_WS('-',SUBSTR(HEX(uuid),1,8),SUBSTR(HEX(uuid),9,4),SUBSTR(HEX(uuid),13,4),SUBSTR(HEX(uuid),17,4),SUBSTR(HEX(uuid),21,12))) FROM user WHERE email='qa-ga-owner@hermes.test'")[0][0]
        s, bten = http("POST", "/tenant", {"businessName": "qa-ga-b", "taxId": f"QAGB{int(time.time()) % 100000000:08d}", "countryCode": "US"}, admin)
        assert s == 201, (s, bten)
        owner_profile = b.system_profile("TenantOwner")

        def grant(extras):
            return http("PUT", f"/access-profile/user/uuid/{owner_uuid}", {"profileUuid": owner_profile, "extraPermissions": extras}, admin)[0]

        # AC-091a -- the tree: the platform's tenants sit under the root; without the permission nobody creates below theirs
        parent_ok = bten.get("parentTenantId") is not None and str(bten["parentTenantId"]) == str(rows("SELECT id FROM tenant WHERE parent_tenant_id IS NULL")[0][0])
        denied = http("POST", "/tenant", {"businessName": "qa-ga-t1", "taxId": "QAGT10000001", "countryCode": "US"}, owner)
        record("AC-091a", parent_ok and named(denied, "tenant.child.create"),
               f"a tenant the platform created has the root as parent: {parent_ok}; the owner without tenant.child.create is refused naming it: {denied[0]} {denied[1].get('permission') if isinstance(denied[1], dict) else ''}")

        # AC-091b -- create below, list the branch, never beside
        g = grant(CHILD)
        s1, t1 = http("POST", "/tenant", {"businessName": "qa-ga-t1", "taxId": f"QAGT{int(time.time()) % 100000000:08d}", "countryCode": "US"}, owner)
        lst = http("GET", "/tenant", token=owner)[1]["items"]
        names = sorted(t["businessName"] for t in lst)
        s_b = http("GET", f"/tenant/uuid/{bten['uuid']}", token=owner)[0]
        s_b2 = call("GET", f"/tenant/uuid/{bten['uuid']}", owner, tenant=bten["uuid"])[0]
        record("AC-091b", g == 204 and s1 == 201 and t1.get("parentTenantId") == t_id and names == ["qa-ga-t1", "qa-ga-tenant"] and s_b == 404 and s_b2 == 404,
               f"grant {g}; created {s1} with parent {t1.get('parentTenantId')} (T is {t_id}); the owner lists {names}; the sibling's record {s_b}, with the header {s_b2}")

        # AC-091c -- read the descendant's operation through the header; not the ancestor, not the sibling
        v = http("POST", "/vehicle", {"plate": "QAGAT1A", "model": "qa-ga-t1", "status": "Active", "tenantId": t1["id"]}, admin)
        own_list = http("GET", "/vehicle", token=owner)[1]
        own_n = len(own_list["items"]) if isinstance(own_list, dict) else -1
        no_read = call("GET", "/vehicle", owner, tenant=t1["uuid"])
        grant(CHILD + ["tenant.child.data.read"])
        read = call("GET", "/vehicle", owner, tenant=t1["uuid"])
        plates = [i["plate"] for i in read[1]["items"]] if read[0] == 200 else []
        from_t1 = call("GET", "/vehicle", b.login("qa-ga-owner@hermes.test", b.PASSWORD), tenant=bten["uuid"])[0]
        self_t = call("GET", "/vehicle", owner, tenant=t_uuid)[0]
        junk = call("GET", "/vehicle", owner, tenant="not-a-uuid")[0]
        record("AC-091c", v[0] == 201 and own_n == 0 and named(no_read, "tenant.child.data.read") and read[0] == 200 and plates == ["QAGAT1A"]
               and from_t1 == 404 and self_t == 200 and junk == 404,
               f"T's own list {own_n}; without data.read {no_read[0]} {no_read[1].get('permission') if isinstance(no_read[1], dict) else ''}; with it {read[0]} {plates}; sibling {from_t1}; own tenant named {self_t}; junk uuid {junk}")

        # AC-091d -- write needs its own permission, and the write needs the read
        vuuid = v[1]["uuid"]
        upd = {"plate": "QAGAT1A", "model": "qa-ga-t1-edited", "status": "Active"}
        no_write = call("PUT", f"/vehicle/uuid/{vuuid}", owner, tenant=t1["uuid"], body=upd)
        grant(CHILD + ["tenant.child.data.write"])
        write_only = call("PUT", f"/vehicle/uuid/{vuuid}", owner, tenant=t1["uuid"], body=upd)
        grant(CHILD + ["tenant.child.data.read", "tenant.child.data.write"])
        wrote = call("PUT", f"/vehicle/uuid/{vuuid}", owner, tenant=t1["uuid"], body=upd)
        model = one(f"SELECT model FROM vehicle WHERE uuid=UNHEX(REPLACE('{vuuid}','-',''))")
        record("AC-091d", named(no_write, "tenant.child.data.write") and named(write_only, "tenant.child.data.read") and wrote[0] in (200, 204) and model == "qa-ga-t1-edited",
               f"without write {no_write[0]} {no_write[1].get('permission') if isinstance(no_write[1], dict) else ''}; write without read {write_only[0]} {write_only[1].get('permission') if isinstance(write_only[1], dict) else ''}; with both {wrote[0]}, stored model {model}")

        # AC-091e -- the descendant's registry and users, and the guard on granting
        upd_t = call("PUT", f"/tenant/uuid/{t1['uuid']}", owner, tenant=t1["uuid"], body={"businessName": "qa-ga-t1 renamed", "taxId": t1["taxId"], "countryCode": "US"})
        mech = call("POST", "/user", owner, tenant=t1["uuid"], body={"email": "qa-ga-t1-user@hermes.test", "name": "t1", "profileUuid": b.system_profile("TenantUser"), "enabled": True})
        in_t1 = one("SELECT tenant_id FROM user WHERE email='qa-ga-t1-user@hermes.test'")
        up = http("GET", "/tenant", token=b.login("qa-ga-owner@hermes.test", b.PASSWORD))[0]
        record("AC-091e", upd_t[0] in (200, 204) and mech[0] == 201 and str(in_t1) == str(t1["id"]),
               f"update the child's registry {upd_t[0]}; create a user inside it {mech[0]}, stored in tenant {in_t1} (T1 is {t1['id']})")

        # AC-091f -- move a tenant in the tree: within the caller's branch only, never into a cycle, never the caller's own
        s2, t2 = http("POST", "/tenant", {"businessName": "qa-ga-t2", "taxId": f"QAGU{int(time.time()) % 100000000:08d}", "countryCode": "US"}, owner)
        reg = lambda t, **extra: {"businessName": t["businessName"], "taxId": t["taxId"], "countryCode": "US", **extra}
        to_t2 = call("PUT", f"/tenant/uuid/{t1['uuid']}", owner, tenant=t1["uuid"], body=reg(t1, parentTenantId=t2["id"]))
        parent_after = one(f"SELECT parent_tenant_id FROM tenant WHERE id={t1['id']}")
        cycle = call("PUT", f"/tenant/uuid/{t2['uuid']}", owner, tenant=t2["uuid"], body=reg(t2, parentTenantId=t1["id"]))
        outside = call("PUT", f"/tenant/uuid/{t1['uuid']}", owner, tenant=t1["uuid"], body=reg(t1, parentTenantId=bten["id"]))
        own_t = call("PUT", f"/tenant/uuid/{t_uuid}", owner, body=reg(t1, parentTenantId=t2["id"]) | {"businessName": "qa-ga-tenant", "taxId": rows(f"SELECT tax_id FROM tenant WHERE id={t_id}")[0][0]})
        back = call("PUT", f"/tenant/uuid/{t1['uuid']}", owner, tenant=t1["uuid"], body=reg(t1, parentTenantId=t_id))
        record("AC-091f", s2 == 201 and to_t2[0] == 200 and str(parent_after) == str(t2["id"]) and cycle[0] == 400 and outside[0] == 403 and own_t[0] == 403
               and back[0] == 200 and str(one(f"SELECT parent_tenant_id FROM tenant WHERE id={t1['id']}")) == str(t_id),
               f"T1 under T2 {to_t2[0]} (parent now {parent_after}); T2 under its own child {cycle[0]}; T1 under the sibling B {outside[0]}; the caller's own tenant {own_t[0]}; back under T {back[0]}")

        # AC-091g -- a profile the parent made is visible and assignable in the child, and only there
        t1_owner = owner_of(admin, t1["id"], "t1-owner")
        shared = http("POST", "/access-profile", {"name": "QA Shared by parent", "permissions": ["vehicle.read"]}, owner)
        seen_child = [p["name"] for p in http("GET", "/access-profile", token=t1_owner)[1]["items"]]
        b_owner = owner_of(admin, bten["id"], "b-owner")
        seen_sibling = [p["name"] for p in http("GET", "/access-profile", token=b_owner)[1]["items"]]
        made = http("POST", "/user", {"email": "qa-ga-inherit@hermes.test", "name": "inherit", "profileUuid": shared[1].get("uuid"), "enabled": True}, t1_owner)
        in_sibling = http("POST", "/user", {"email": "qa-ga-inherit2@hermes.test", "name": "inherit", "profileUuid": shared[1].get("uuid"), "enabled": True}, b_owner)
        edit = http("PUT", f"/access-profile/uuid/{shared[1].get('uuid')}", {"name": "QA Shared by parent", "permissions": ["vehicle.read", "vehicle.update"], "active": True}, t1_owner)
        record("AC-091g", shared[0] == 201 and "QA Shared by parent" in seen_child and "QA Shared by parent" not in seen_sibling and made[0] == 201
               and in_sibling[0] in (404, 409) and edit[0] in (403, 404),
               f"the child sees {'QA Shared by parent' in seen_child}, the sibling sees {'QA Shared by parent' in seen_sibling}; the child hands it out {made[0]}; the sibling {in_sibling[0]}; the child edits the parent's profile {edit[0]}")

        # AC-096 -- the trail: the affected tenant sees what the support did in it, the actor what it did, a stranger nothing
        seen_t1 = http("GET", "/cross-tenant-action", token=t1_owner)
        seen_t = http("GET", "/cross-tenant-action", token=owner)
        seen_b = http("GET", "/cross-tenant-action", token=b_owner)
        rows_t1 = seen_t1[1]["items"] if seen_t1[0] == 200 else []
        writes = [r for r in rows_t1 if r["kind"] == "write"]
        reads = [r for r in rows_t1 if r["kind"] == "read"]
        one_write = next((r for r in writes if r["method"] == "PUT" and r["path"].startswith("/vehicle/uuid/")), {})
        record("AC-096", seen_t1[0] == 200 and one_write.get("actorEmail") == "qa-ga-owner@hermes.test"
               and one_write.get("actorTenantId") == t_id and one_write.get("affectedTenantId") == t1["id"]
               and one_write.get("operation") == "/vehicle/uuid/{uuid}" and one_write.get("occurredAt") and reads
               and seen_t[0] == 200 and seen_t[1]["totalItems"] >= len(rows_t1)
               and seen_b[0] == 200 and seen_b[1]["totalItems"] == 0,
               f"T1 sees {len(rows_t1)} actions ({len(writes)} writes, {len(reads)} reads); the vehicle edit: actor {one_write.get('actorEmail')} from tenant {one_write.get('actorTenantId')} into {one_write.get('affectedTenantId')} {one_write.get('operation')} at {one_write.get('occurredAt')}; "
               f"the actor T lists {seen_t[1]['totalItems'] if seen_t[0] == 200 else seen_t[0]}; the sibling B sees {seen_b[1]['totalItems'] if seen_b[0] == 200 else seen_b[0]}")
    finally:
        b.cleanup()
    bad = [k for k, v in b.RESULTS.items() if v["status"] != "PASS"]
    print(f"{len(b.RESULTS) - len(bad)}/{len(b.RESULTS)} scenarios PASS")
    return 1 if bad else 0


if __name__ == "__main__":
    try:
        sys.exit(run())
    except Exception as error:
        print(f"QA BLOCKED: {type(error).__name__}: {error}", file=sys.stderr)
        b.cleanup()
        sys.exit(2)
