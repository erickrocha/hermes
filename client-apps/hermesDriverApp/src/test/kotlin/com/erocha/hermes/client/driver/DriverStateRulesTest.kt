package com.erocha.hermes.client.driver

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.TimeZone

class DriverStateRulesTest {
    private val saoPaulo = TimeZone.getTimeZone("America/Sao_Paulo")

    @Test
    fun theReceiptsPrintedLocalTimeBecomesTheServersUtcInstant() {
        // 14:32 in Sao Paulo (UTC-3, no DST in 2026) is 17:32 UTC.
        assertEquals("2026-10-27T17:32:10", recordedAtUtc("2026-10-27T14:32:10", saoPaulo))
    }

    @Test
    fun aMissingOrFutureTimeIsStampedNow() {
        val before = System.currentTimeMillis()
        val nowLike = recordedAtUtc(null, saoPaulo)
        val future = recordedAtUtc("2999-01-01T10:00:00", saoPaulo)
        assertTrue(nowLike.startsWith(Instants.nowUtc().take(10)) || before > 0)
        assertTrue(future.startsWith(Instants.nowUtc().take(4)))
    }

    @Test
    fun scannedNumbersAreShownTheWayTheDriverTypesThem() {
        assertEquals("45,23", formatLiters(45.23))
        assertEquals("50", formatLiters(50.0))
        assertEquals("266,81", formatMoneyField(26681))
        assertEquals("100,00", formatMoneyField(10000))
    }
}

class ReceiptFormTest {
    private val vehicle = com.erocha.hermes.client.shared.Vehicle(uuid = "v1", plate = "AAA0A00", prefix = "2424")

    @Test
    fun aReadReceiptFillsTheFieldsTheDriverThenConfirms() {
        val form = ReceiptForm(vehicle)
        form.apply(com.erocha.hermes.client.shared.ScannedReceipt(45.23, 26681, "POSTO BOM", "2026-10-27T14:32:10"))
        assertEquals("45,23", form.liters)
        assertEquals("266,81", form.value)
        assertEquals("POSTO BOM", form.station)
        assertEquals("2026-10-27T14:32:10", form.printedAt)
        assertEquals(false, form.scanFailed)
        assertEquals("2424", form.prefix)
        assertTrue(form.canSubmit)
    }

    @Test
    fun aReceiptWithNeitherLitersNorValueIsReportedAsUnreadAndLeavesTheFieldsAlone() {
        val form = ReceiptForm(vehicle).also { it.liters = "10" }
        form.apply(com.erocha.hermes.client.shared.ScannedReceipt(station = "POSTO"))
        assertTrue(form.scanFailed)
        assertEquals("10", form.liters)
    }
}
