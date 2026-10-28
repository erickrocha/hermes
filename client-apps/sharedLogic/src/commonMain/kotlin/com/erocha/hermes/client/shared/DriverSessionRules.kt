package com.erocha.hermes.client.shared

const val DEFAULT_REFRESH_MARGIN_SECONDS = 300L

/** A grant limited to the holder's own records: `code@own`. */
const val OWN_SUFFIX = "@own"

// Permission codes this app gates on; the names follow `backend/business/src/domain/permission.rs`.
/** The driver gate: the holder may be named as a vehicle's, trip's, checklist's or fuelling's driver. */
const val DRIVER_ACT = "driver.act"
const val CHECKLIST_RUN_CREATE = "checklist-run.create"
const val FUEL_ENTRY_READ = "fuel-entry.read"
const val FUEL_ENTRY_REGISTER_RECEIPT = "fuel-entry.register-receipt"
const val VEHICLE_ODOMETER_RECORD = "vehicle.odometer.record"
const val VEHICLE_TRACKING_READ = "vehicle.tracking.read"
const val EXTRA_TRIP_READ = "extra-trip.read"
const val GARAGE_TRIAGE_OPEN = "garage.triage.open"
const val GARAGE_TRIAGE_MARK_SERVICE = "garage.triage.mark-service"
const val TECHNICAL_INSPECTION_CREATE = "technical-inspection.create"
const val WORK_ORDER_ITEM_ADD = "work-order.item.add"
const val WORK_ORDER_ITEM_RESOLVE = "work-order.item.resolve"
const val WORK_ORDER_ISSUE_PART = "work-order.issue-part"

/** Holds [code] in any scope: the full grant or the `@own` one. */
fun has(permissions: Collection<String>, code: String): Boolean = code in permissions || "$code$OWN_SUFFIX" in permissions

/** Holds the full grant, not merely the own-records one. */
fun hasFull(permissions: Collection<String>, code: String): Boolean = code in permissions

/** Holds [code] over the holder's own records only. */
fun onlyOwn(permissions: Collection<String>, code: String): Boolean =
    "$code$OWN_SUFFIX" in permissions && code !in permissions

/**
 * What the signed-in person may do in this app, read once from the session's grants. Interface courtesy only;
 * the server decides authorization on every call (HRMS-505).
 */
data class AppAccess(val permissions: List<String>) {
    fun has(code: String): Boolean = has(permissions, code)
    fun onlyOwn(code: String): Boolean = onlyOwn(permissions, code)

    /** The driver's own screens: vehicles, checklists, trips, fuellings. */
    val driver: Boolean get() = has(DRIVER_ACT)

    // Mechanic screens. Each is gated by what it writes, never by a read grant: the Driver profile reads the garage
    // queue and the work orders too, and must not land on the mechanic's home for that.
    val garage: Boolean get() = has(GARAGE_TRIAGE_MARK_SERVICE) || has(GARAGE_TRIAGE_OPEN)
    val valeta: Boolean get() = has(TECHNICAL_INSPECTION_CREATE)
    val orders: Boolean get() = has(WORK_ORDER_ITEM_ADD) || has(WORK_ORDER_ITEM_RESOLVE) || has(WORK_ORDER_ISSUE_PART)
    val mechanic: Boolean get() = garage || valeta || orders

    /** The app serves drivers and mechanics; anyone else is turned away at sign-in. */
    val admitted: Boolean get() = driver || mechanic

    val checklist: Boolean get() = has(CHECKLIST_RUN_CREATE)
    val fuelReceipt: Boolean get() = has(FUEL_ENTRY_REGISTER_RECEIPT)
    val fuelEntries: Boolean get() = has(FUEL_ENTRY_READ) || fuelReceipt
    val odometer: Boolean get() = has(VEHICLE_ODOMETER_RECORD)
    val position: Boolean get() = has(VEHICLE_TRACKING_READ)
    val trips: Boolean get() = has(EXTRA_TRIP_READ)

    val openTriage: Boolean get() = has(GARAGE_TRIAGE_OPEN)
    val markService: Boolean get() = has(GARAGE_TRIAGE_MARK_SERVICE)
    val addOrderItem: Boolean get() = has(WORK_ORDER_ITEM_ADD)
    val resolveOrderItem: Boolean get() = has(WORK_ORDER_ITEM_RESOLVE)
    val issuePart: Boolean get() = has(WORK_ORDER_ISSUE_PART)

    companion object {
        val NONE = AppAccess(emptyList())
    }
}

val DriverSession.access: AppAccess get() = AppAccess(permissions)

/** [expireIn] is an absolute Unix instant in seconds; renew once it is within [marginSeconds]. */
fun shouldRefresh(expireIn: Long, nowSeconds: Long, marginSeconds: Long = DEFAULT_REFRESH_MARGIN_SECONDS): Boolean =
    nowSeconds >= expireIn - marginSeconds

/** Seconds to wait before the next renewal; zero when it is already due. */
fun secondsUntilRefresh(expireIn: Long, nowSeconds: Long, marginSeconds: Long = DEFAULT_REFRESH_MARGIN_SECONDS): Long =
    maxOf(0L, expireIn - marginSeconds - nowSeconds)
