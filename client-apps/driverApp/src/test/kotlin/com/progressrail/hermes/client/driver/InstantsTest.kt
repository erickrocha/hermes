package com.progressrail.hermes.client.driver

import org.junit.Assert.assertEquals
import org.junit.Test
import java.util.Date
import java.util.TimeZone

class InstantsTest {
    private val saoPaulo = TimeZone.getTimeZone("America/Sao_Paulo")

    @Test
    fun serverInstantsAreShownInTheDevicesLocalTime() {
        assertEquals("15/10/2026 09:00", Instants.toLocal("2026-10-15T12:00:00Z", saoPaulo))
        assertEquals("15/10/2026 09:00", Instants.toLocal("2026-10-15T12:00:00", saoPaulo))
        assertEquals("15/10/2026 09:00", Instants.toLocal("2026-10-15T12:00:00.123456Z", saoPaulo))
        assertEquals("", Instants.toLocal(null, saoPaulo))
        assertEquals("not a date", Instants.toLocal("not a date", saoPaulo))
    }

    @Test
    fun theWireInstantIsUtcWithoutZone() {
        assertEquals("2026-10-15T12:00:00", Instants.nowUtc(Date(1_792_065_600_000L)))
    }

    @Test
    fun businessDatesAndMoneyAreFormattedForReading() {
        assertEquals("20/11/2026", Instants.date("2026-11-20"))
        assertEquals("", Instants.date(null))
        assertEquals("08:00", Instants.time("08:00:00"))
        assertEquals("", Instants.time(null))
        assertEquals("123,45", Instants.money(12345))
        assertEquals("1.234,50", Instants.money(123450))
    }
}
