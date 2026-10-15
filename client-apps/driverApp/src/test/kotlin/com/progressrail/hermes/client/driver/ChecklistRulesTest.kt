package com.progressrail.hermes.client.driver

import com.progressrail.hermes.client.shared.ANSWER_CONFORMING
import com.progressrail.hermes.client.shared.ANSWER_NON_CONFORMING
import com.progressrail.hermes.client.shared.CHECKLIST_DEPARTURE
import com.progressrail.hermes.client.shared.CHECKLIST_RETURN
import com.progressrail.hermes.client.shared.ChecklistRun
import com.progressrail.hermes.client.shared.ChecklistTemplate
import com.progressrail.hermes.client.shared.ChecklistTemplateItem
import com.progressrail.hermes.client.shared.DriverSession
import com.progressrail.hermes.client.shared.VehicleAction
import com.progressrail.hermes.client.shared.buildChecklistRun
import com.progressrail.hermes.client.shared.canSubmitChecklist
import com.progressrail.hermes.client.shared.decodeDriverSession
import com.progressrail.hermes.client.shared.defaultTemplate
import com.progressrail.hermes.client.shared.encode
import com.progressrail.hermes.client.shared.parseOdometer
import com.progressrail.hermes.client.shared.vehicleAction
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ChecklistRulesTest {
    private val items = listOf(ChecklistTemplateItem("i1", "Freios"), ChecklistTemplateItem("i2", "Buzina"))
    private fun template(type: String = CHECKLIST_DEPARTURE, active: Boolean = true, isDefault: Boolean = true, uuid: String = "t1") =
        ChecklistTemplate(uuid, "Modelo", type, active, isDefault, items)

    @Test
    fun theNextActionComesFromThePossessionTheServerReports() {
        assertEquals(VehicleAction.Depart, vehicleAction(null, "me"))
        val mine = ChecklistRun(uuid = "run-1", driverUuid = "me", checklistType = CHECKLIST_DEPARTURE)
        assertEquals(VehicleAction.Return("run-1"), vehicleAction(mine, "me"))
        assertEquals(VehicleAction.HeldByAnother, vehicleAction(mine.copy(driverUuid = "other"), "me"))
        // Without a known own uuid nothing may be treated as the driver's own possession.
        assertEquals(VehicleAction.HeldByAnother, vehicleAction(mine, "other"))
    }

    @Test
    fun onlyTheActiveDefaultOfTheRightTypeIsUsed() {
        val all = listOf(
            template(isDefault = false, uuid = "plain"),
            template(active = false, uuid = "inactive"),
            template(type = CHECKLIST_RETURN, uuid = "return"),
            template(uuid = "departure-default"),
        )
        assertEquals("departure-default", defaultTemplate(all, CHECKLIST_DEPARTURE)?.uuid)
        assertEquals("return", defaultTemplate(all, CHECKLIST_RETURN)?.uuid)
        assertNull(defaultTemplate(all.take(2), CHECKLIST_DEPARTURE))
    }

    @Test
    fun theReadingAcceptsACommaAndRefusesNonNumbers() {
        assertEquals(12345.6, parseOdometer("12345,6")!!, 0.0)
        assertEquals(1000.0, parseOdometer(" 1000 ")!!, 0.0)
        assertNull(parseOdometer(""))
        assertNull(parseOdometer("abc"))
        assertNull(parseOdometer("NaN"))
    }

    @Test
    fun sendNeedsAReadingAndEveryItemAnswered() {
        val all = mapOf("i1" to ANSWER_CONFORMING, "i2" to ANSWER_NON_CONFORMING)
        assertTrue(canSubmitChecklist("100", items, all))
        assertFalse(canSubmitChecklist("", items, all))
        assertFalse(canSubmitChecklist("x", items, all))
        assertFalse(canSubmitChecklist("100", items, mapOf("i1" to ANSWER_CONFORMING)))
        assertFalse(canSubmitChecklist("100", emptyList(), all))
    }

    @Test
    fun theRunIsBuiltAsTheDriverWithTheOpeningOfAReturn() {
        val answers = mapOf("i1" to ANSWER_CONFORMING, "i2" to ANSWER_NON_CONFORMING)
        val run = buildChecklistRun(
            template(CHECKLIST_RETURN), "me", "v1", VehicleAction.Return("run-1"), 1100.0, answers, mapOf("i2" to " ruim ", "i1" to " "), "  ",
        )
        assertEquals("me", run.driverUuid)
        assertEquals("v1", run.vehicleUuid)
        assertEquals("t1", run.checklistTemplateUuid)
        assertEquals(CHECKLIST_RETURN, run.checklistType)
        assertEquals("run-1", run.openingChecklistUuid)
        assertNull(run.notes)
        assertEquals(listOf("i1", "i2"), run.answers.map { it.checklistTemplateItemUuid })
        assertNull(run.answers[0].observation)
        assertEquals("ruim", run.answers[1].observation)
        assertNull(buildChecklistRun(template(), "me", "v1", VehicleAction.Depart, 1.0, answers, emptyMap(), "").openingChecklistUuid)
    }

    @Test
    fun aSessionSavedBeforeThisChangeStillDecodesWithoutAUuid() {
        val old = """{"accessToken":"a","refreshToken":"r","expireIn":1792000000,"name":"Maria","role":"Driver"}"""
        val decoded = decodeDriverSession(old)
        assertEquals("", decoded?.uuid)
        val withUuid = DriverSession("a", "r", 1L, "Maria", "Driver", "u-1")
        assertEquals(withUuid, decodeDriverSession(withUuid.encode()))
    }
}
