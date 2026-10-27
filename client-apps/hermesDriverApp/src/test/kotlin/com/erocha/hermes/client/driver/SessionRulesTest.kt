package com.erocha.hermes.client.driver

import com.erocha.hermes.client.shared.DriverSession
import com.erocha.hermes.client.shared.decodeDriverSession
import com.erocha.hermes.client.shared.encode
import com.erocha.hermes.client.shared.isDriverRole
import com.erocha.hermes.client.shared.isFieldRole
import com.erocha.hermes.client.shared.isMechanicRole
import com.erocha.hermes.client.shared.secondsUntilRefresh
import com.erocha.hermes.client.shared.shouldRefresh
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class SessionRulesTest {
    @Test
    fun onlyTheDriverAndMechanicRolesAreAccepted() {
        assertTrue(isDriverRole("Driver") && !isMechanicRole("Driver"))
        assertTrue(isMechanicRole("Mechanic") && !isDriverRole("Mechanic"))
        assertTrue(isFieldRole("Driver") && isFieldRole("Mechanic"))
        listOf("TenantOwner", "TenantUser", "SysAdmin", "driver", "mechanic", "").forEach {
            assertFalse(it, isFieldRole(it))
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
        val session = DriverSession("a", "r", 123, "Maria", "Driver")
        assertEquals(session, decodeDriverSession(session.encode()))
        assertNull(decodeDriverSession("not json"))
        assertNull(decodeDriverSession("""{"accessToken":"a"}"""))
    }
}
