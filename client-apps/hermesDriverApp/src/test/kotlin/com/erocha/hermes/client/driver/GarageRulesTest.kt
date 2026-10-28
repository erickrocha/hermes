package com.erocha.hermes.client.driver

import com.erocha.hermes.client.shared.GarageService
import org.junit.Assert.assertEquals
import org.junit.Test

class GarageRulesTest {
    private val services = listOf(
        GarageService("1", "WC", "Internal", requiredForDeparture = false, state = "Performed", effectiveState = "Performed"),
        GarageService("2", "Diesel", "External", requiredForDeparture = true, state = "Pending", effectiveState = "Pending"),
        GarageService("3", "ARLA", "External", requiredForDeparture = false, state = "Performed", effectiveState = "Pending"),
    )

    @Test
    fun aGroupFilterKeepsOnlyThatGroupAndAllKeepsEverything() {
        assertEquals(listOf("Diesel", "ARLA"), GarageRules.visible(services, GarageRules.EXTERNAL).map { it.name })
        assertEquals(listOf("WC"), GarageRules.visible(services, GarageRules.INTERNAL).map { it.name })
        assertEquals(3, GarageRules.visible(services, GarageRules.ALL).size)
    }

    @Test
    fun attendedFollowsTheEffectiveStateNotTheMarkedOne() {
        // ARLA was marked performed, but its validity lapsed: it reads Pending again, so the mechanic still owes it.
        assertEquals(1, GarageRules.attended(services))
    }

    @Test
    fun requiredServicesComeFirst() {
        assertEquals(listOf("Diesel", "WC", "ARLA"), GarageRules.ordered(services).map { it.name })
    }
}
