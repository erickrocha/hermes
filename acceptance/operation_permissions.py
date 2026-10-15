#!/usr/bin/env python3
"""EPIC-AC-03-S02 / AC-02-S04: generate business/src/domain/operation_permission.rs -- which catalogue permission each
operation (METHOD + route template) asks for -- from the reviewed inventory's operation lists plus the rename table
that turns the scan's raw names into the catalogue's domain names. Stdlib only.   usage: operation_permissions.py > file.rs
"""
import os, re
ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "02-system_requirements", "hermes")
RENAME = {
    "cities.by-province.read": "reference-data.city.read", "cities.read": "reference-data.city.read", "city.read": "reference-data.city.read",
    "city.create": "reference-data.city.create", "city.import": "reference-data.city.import", "country.read": "reference-data.country.read",
    "province.create": "reference-data.province.create", "province.import": "reference-data.province.import",
    "province.page.read": "reference-data.province.manage-read", "province.read": "reference-data.province.read",
    "customer.day-offs.read": "customer.day-off.read", "extra-trip.import.preview": "extra-trip.import-preview",
    "fuel-entry.average": "fuel.consumption-average.read", "fuel-entry.gauge.read": "fuel.tank-gauge.read",
    "fuel-entry.receipt": "fuel-entry.register-receipt", "fuel-entry.report.read": "fuel.report.read", "fuel-entry.sync": "fuel-entry.sync-provider",
    "garage-attendance.active": "garage.triage.read", "garage-attendance.call-list": "garage.call-list.read",
    "garage-attendance.checkout": "garage.triage.check-out", "garage-attendance.create": "garage.triage.open",
    "garage-attendance.monitor": "garage.monitor.read", "garage-attendance.monitor.cards": "garage.monitor-cards.read",
    "garage-attendance.monitor.matrix": "garage.monitor-matrix.read", "garage-attendance.queue": "garage.queue.read",
    "garage-attendance.read": "garage.triage.read", "garage-attendance.service.update": "garage.triage.mark-service",
    "garage-call.cancel": "garage.call.cancel", "garage-call.create": "garage.call.create", "garage-call.read": "garage.call.read",
    "garage-service.create": "garage.service-catalogue.create", "garage-service.read": "garage.service-catalogue.read",
    "garage-service.update": "garage.service-catalogue.update",
    "part.adjustment.create": "part.stock-adjust", "part.entry.create": "part.stock-entry",
    "preventive-plan.alert": "preventive-plan.alert.read", "preventive-plan.alert.discharge.create": "preventive-plan.alert.discharge",
    "preventive-plan.extension.create": "preventive-plan.extend", "preventive-plan.work-order.create": "preventive-plan.open-work-order",
    "purchase-order.mark-ordered.update": "purchase-order.mark-ordered", "purchase-order.mark-purchased.update": "purchase-order.mark-purchased",
    "transport-demand.allocations.read": "transport-demand.allocation.read", "transport-demand.daily-schedules.read": "transport-demand.daily-schedule.read",
    "transport-demand.exceptions.read": "transport-demand.exception.read",
    "user.change-password": "user.change-own-password", "user.read": "user.read-own", "vehicle-presence.create": "vehicle-presence.publish",
    "vehicle.assignment.create": "vehicle.assign-driver", "vehicle.assignment.end": "vehicle.end-assignment",
    "vehicle.assignments.read": "vehicle.assignment.read", "vehicle.km-evolution.create": "vehicle.odometer.record",
    "vehicle.km-evolutions.read": "vehicle.odometer.read", "work-order.item.create": "work-order.item.add", "work-order.postings.read": "work-order.posting.read",
}
PUBLIC = {"accept-invite.create", "login.create", "refresh.create"}
# The profile operations post-date the scan; the owner administers them (D-27 3).
PROFILE = [("GET", "/access-profile/catalogue", "profile.read"), ("GET", "/access-profile", "profile.read"),
           ("GET", "/access-profile/uuid/{uuid}", "profile.read"), ("POST", "/access-profile", "profile.create"),
           ("PUT", "/access-profile/uuid/{uuid}", "profile.update"),
           ("GET", "/access-profile/user/uuid/{user_uuid}", "profile.read"), ("PUT", "/access-profile/user/uuid/{user_uuid}", "profile.assign")]
GARAGE = [("POST", "/garage-attendance/uuid/{uuid}/release-with-pendency", "garage.triage.check-out")]

# C-030 EPIC-SP-07/08 (HRMS-804...810) post-date the scan too.
COST = [("GET", "/cost-report/vehicle/uuid/{uuid}", "cost-report.vehicle.read"), ("GET", "/cost-report/vehicle/uuid/{uuid}/export", "cost-report.vehicle.read"),
        ("GET", "/cost-report/work-order/uuid/{uuid}", "cost-report.work-order.read"), ("GET", "/cost-report/suppliers", "cost-report.supplier.read"),
        ("GET", "/cost-report/supplier-closing", "cost-report.supplier.read"), ("GET", "/cost-report/supplier-closing/export", "cost-report.supplier.read")]
# C-031 (the tenant's default checklist template) and an owner-only trip edit that the scan missed.
LATER = [("PUT", "/checklist-template/uuid/{uuid}/default", "checklist-template.create"),
         ("POST", "/extra-trip/uuid/{uuid}/preparation-skip", "extra-trip.update")]
table = list(PROFILE) + COST + GARAGE + LATER
for line in open(os.path.join(ROOT, "access-model_permission_inventory.md"), encoding="utf-8"):
    m = re.match(r"\| `([^`]+)` \| (.+?) \| ", line)
    if not m or m.group(1) in PUBLIC:
        continue
    for op in re.findall(r"`((?:GET|POST|PUT|DELETE) [^`]+)`", m.group(2)):
        method, path = op.split(" ", 1)
        table.append((method, path, RENAME.get(m.group(1), m.group(1))))
print("//! Generated by `acceptance/operation_permissions.py` from the reviewed inventory; edit the script, not this.\n"
      "//! Which catalogue permission each operation asks for (`HRMS-856`), so a refusal can name it (`HRMS-859`).\n")
print("pub const OPERATIONS: &[(&str, &str, &str)] = &[")
for method, path, code in sorted(set(table), key=lambda r: (r[1], r[0])):
    print(f'    ("{method}", "{path}", "{code}"),')
print("];")
