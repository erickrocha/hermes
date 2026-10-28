package com.erocha.hermes.client.driver

import com.erocha.hermes.client.shared.GarageService
import com.erocha.hermes.client.shared.STATE_PENDING

/** What the mechanic's lists show; the server stays the authority on every state (`TRM-447`). */
internal object GarageRules {
    const val ALL = "All"
    const val EXTERNAL = "External"
    const val INTERNAL = "Internal"

    fun visible(services: List<GarageService>, group: String): List<GarageService> =
        if (group == ALL) services else services.filter { it.serviceGroup == group }

    /** Services no longer waiting for the mechanic (performed, not needed or not done). */
    fun attended(services: List<GarageService>): Int = services.count { it.effectiveState != STATE_PENDING }

    /** Required services first, then the catalogue order the server already sent. */
    fun ordered(services: List<GarageService>): List<GarageService> = services.sortedBy { !it.requiredForDeparture }
}
