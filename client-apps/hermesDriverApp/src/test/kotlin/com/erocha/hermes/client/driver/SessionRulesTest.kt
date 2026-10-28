package com.erocha.hermes.client.driver

import com.erocha.hermes.client.shared.AppAccess
import com.erocha.hermes.client.shared.CHECKLIST_RUN_CREATE
import com.erocha.hermes.client.shared.DRIVER_ACT
import com.erocha.hermes.client.shared.DriverSession
import com.erocha.hermes.client.shared.EXTRA_TRIP_READ
import com.erocha.hermes.client.shared.FUEL_ENTRY_REGISTER_RECEIPT
import com.erocha.hermes.client.shared.GARAGE_TRIAGE_MARK_SERVICE
import com.erocha.hermes.client.shared.GARAGE_TRIAGE_OPEN
import com.erocha.hermes.client.shared.TECHNICAL_INSPECTION_CREATE
import com.erocha.hermes.client.shared.VEHICLE_ODOMETER_RECORD
import com.erocha.hermes.client.shared.WORK_ORDER_ISSUE_PART
import com.erocha.hermes.client.shared.WORK_ORDER_ITEM_ADD
import com.erocha.hermes.client.shared.WORK_ORDER_ITEM_RESOLVE
import com.erocha.hermes.client.shared.access
import com.erocha.hermes.client.shared.decodeDriverSession
import com.erocha.hermes.client.shared.encode
import com.erocha.hermes.client.shared.has
import com.erocha.hermes.client.shared.hasFull
import com.erocha.hermes.client.shared.onlyOwn
import com.erocha.hermes.client.shared.secondsUntilRefresh
import com.erocha.hermes.client.shared.shouldRefresh
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SessionRulesTest {
    /** The Driver system profile's grants, as the backend seeds them: own-records scope where the catalogue allows it. */
    private val driverGrants = listOf(
        DRIVER_ACT, "vehicle.read@own", "checklist-run.create@own", "vehicle.odometer.record@own",
        "fuel-entry.read@own", "fuel-entry.register-receipt@own", "vehicle.tracking.read@own", "extra-trip.read@own",
        // Read-only garage and workshop grants the Driver profile also holds; they must not make a mechanic of it.
        "garage.queue.read", "garage.triage.read", "inspection-model.read", "work-order.read",
    )
    private val mechanicGrants = listOf(
        "vehicle.read", "garage.queue.read", "garage.triage.read", GARAGE_TRIAGE_OPEN, GARAGE_TRIAGE_MARK_SERVICE,
        "inspection-model.read", TECHNICAL_INSPECTION_CREATE, "work-order.read", WORK_ORDER_ITEM_ADD,
        WORK_ORDER_ITEM_RESOLVE, WORK_ORDER_ISSUE_PART, "part.read",
    )

    @Test
    fun anOwnGrantCountsForHasButNotForHasFull() {
        val grants = listOf("vehicle.read@own", "checklist-run.create")
        assertTrue(has(grants, "vehicle.read"))
        assertTrue(has(grants, "checklist-run.create"))
        assertFalse(has(grants, "fuel-entry.read"))
        assertFalse(hasFull(grants, "vehicle.read"))
        assertTrue(hasFull(grants, "checklist-run.create"))
        assertTrue(onlyOwn(grants, "vehicle.read"))
        assertFalse(onlyOwn(grants, "checklist-run.create"))
        assertFalse(onlyOwn(grants, "fuel-entry.read"))
        // The suffix is exact: a code is never matched by prefix.
        assertFalse(has(listOf("vehicle.read-all"), "vehicle.read"))
    }

    @Test
    fun aDriverIsAdmittedToTheDriverHomeWithEveryDriverFeature() {
        val access = AppAccess(driverGrants)
        assertTrue(access.admitted)
        assertTrue(access.driver)
        assertFalse(access.mechanic)
        assertTrue(access.checklist && access.fuelReceipt && access.fuelEntries && access.odometer && access.position && access.trips)
        assertFalse(access.garage || access.valeta || access.orders)
        assertTrue(access.onlyOwn(CHECKLIST_RUN_CREATE))
    }

    @Test
    fun aMechanicWithoutDriverActIsAdmittedToTheGarageScreens() {
        val access = AppAccess(mechanicGrants)
        assertTrue(access.admitted)
        assertFalse(access.driver)
        assertTrue(access.mechanic)
        assertTrue(access.garage && access.valeta && access.orders)
        assertTrue(access.openTriage && access.markService && access.addOrderItem && access.resolveOrderItem && access.issuePart)
        assertFalse(access.checklist || access.fuelReceipt || access.trips)
    }

    @Test
    fun eachMechanicTabFollowsItsOwnWritePermission() {
        assertTrue(AppAccess(listOf(GARAGE_TRIAGE_MARK_SERVICE)).let { it.garage && !it.openTriage && !it.valeta && !it.orders })
        assertTrue(AppAccess(listOf(GARAGE_TRIAGE_OPEN)).let { it.garage && it.openTriage && !it.markService })
        assertTrue(AppAccess(listOf(TECHNICAL_INSPECTION_CREATE)).let { it.valeta && !it.garage && !it.orders })
        assertTrue(AppAccess(listOf(WORK_ORDER_ITEM_RESOLVE)).let { it.orders && !it.addOrderItem && !it.issuePart })
    }

    @Test
    fun aDriverFeatureIsOfferedOnlyWithItsPermissionInAnyScope() {
        val bare = AppAccess(listOf(DRIVER_ACT))
        assertTrue(bare.admitted && bare.driver)
        assertFalse(bare.checklist || bare.fuelReceipt || bare.fuelEntries || bare.odometer || bare.position || bare.trips)
        val full = AppAccess(listOf(DRIVER_ACT, CHECKLIST_RUN_CREATE, FUEL_ENTRY_REGISTER_RECEIPT, VEHICLE_ODOMETER_RECORD, EXTRA_TRIP_READ))
        assertTrue(full.checklist && full.fuelReceipt && full.fuelEntries && full.odometer && full.trips)
        assertFalse(full.position)
    }

    @Test
    fun anAccountWithNeitherDriverActNorAGaragePermissionIsRefused() {
        listOf(
            emptyList(),
            listOf("user.create", "profile.assign", "vehicle.read", "tenant.update"),
            // Read-only grants over the garage and the workshop do not open any screen of this app.
            listOf("garage.queue.read", "garage.triage.read", "work-order.read", "inspection-model.read"),
            // The driver features without the driver gate are not enough either.
            listOf(CHECKLIST_RUN_CREATE, FUEL_ENTRY_REGISTER_RECEIPT, EXTRA_TRIP_READ),
        ).forEach { grants ->
            val access = AppAccess(grants)
            assertFalse(grants.toString(), access.admitted)
            assertFalse(access.driver || access.mechanic)
        }
    }

    @Test
    fun expireInIsAnAbsoluteInstantAndRenewalStartsBeforeIt() {
        val expireIn = 10_000L
        assertFalse(shouldRefresh(expireIn, nowSeconds = 9_000, marginSeconds = 300))
        assertTrue(shouldRefresh(expireIn, nowSeconds = 9_700, marginSeconds = 300))
        assertTrue(shouldRefresh(expireIn, nowSeconds = 12_000, marginSeconds = 300))
    }

    @Test
    fun waitingTimeNeverGoesNegative() {
        assertEquals(700L, secondsUntilRefresh(10_000, 9_000, 300))
        assertEquals(0L, secondsUntilRefresh(10_000, 12_000, 300))
    }

    @Test
    fun theStoredSessionRoundTripsAndRejectsGarbage() {
        val session = DriverSession("a", "r", 123, "Maria", driverGrants, "u-1", "Motorista")
        assertEquals(session, decodeDriverSession(session.encode()))
        assertTrue(decodeDriverSession(session.encode())!!.access.driver)
        assertNull(decodeDriverSession("not json"))
        assertNull(decodeDriverSession("""{"accessToken":"a"}"""))
    }

    @Test
    fun aSessionSavedWithARoleAndNoGrantsIsDiscardedSoThePersonSignsInAgain() {
        val old = """{"accessToken":"a","refreshToken":"r","expireIn":1792000000,"name":"Maria","role":"Driver","uuid":"u-1"}"""
        assertNull(decodeDriverSession(old))
        // Grants with an unknown extra field still decode: the store stays tolerant of what it does not know.
        val withExtra = """{"accessToken":"a","refreshToken":"r","expireIn":1,"name":"Maria","permissions":["driver.act"],"role":"Driver"}"""
        assertEquals(listOf("driver.act"), decodeDriverSession(withExtra)?.permissions)
    }
}
