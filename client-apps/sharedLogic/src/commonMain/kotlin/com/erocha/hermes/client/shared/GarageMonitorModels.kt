package com.erocha.hermes.client.shared

import kotlinx.serialization.Serializable

@Serializable
data class GarageMonitorEntry(
    val attendanceUuid: String? = null,
    val vehicleUuid: String,
    val prefix: String,
    val manualPriority: Int? = null,
    val nextDeparture: String? = null,
    val requiredPending: List<String> = emptyList(),
    val fillTankAlert: Boolean = false,
    val calledByManager: Boolean = false,
    val readiness: String? = null,
    val urgent: Boolean = false,
)

@Serializable
data class GarageMonitorCard(
    val vehicleUuid: String,
    val prefix: String,
    val tankPercent: Double? = null,
    val needsRefuel: Boolean = false,
    val maintenanceStatus: String? = null,
    val lastGarageCycleDistanceKm: Double? = null,
)

@Serializable
data class GarageMonitorCards(
    val cards: List<GarageMonitorCard> = emptyList(),
    val notShown: Int = 0,
)

@Serializable
data class GarageCallListEntry(
    val vehicleUuid: String,
    val prefix: String,
    val nextDeparture: String? = null,
    val calledByManager: Boolean = false,
    val hasDepartureAhead: Boolean = false,
    val tankCalls: Boolean = false,
)

@Serializable
data class GarageMatrix(
    val columns: List<GarageMatrixColumn> = emptyList(),
    val rows: List<GarageMatrixRow> = emptyList(),
    val rowsNotShown: Int = 0,
    val columnsNotShown: Int = 0,
)

@Serializable
data class GarageMatrixColumn(
    val serviceModelUuid: String,
    val name: String,
    val internal: Boolean,
)

@Serializable
data class GarageMatrixRow(
    val vehicleUuid: String,
    val prefix: String,
    val cells: List<GarageMatrixCell?> = emptyList(),
)

@Serializable
data class GarageMatrixCell(
    val effectiveState: String,
    val stamp: String? = null,
    val lastPerformedAt: String? = null,
)
@Serializable
data class GarageMapVehicle(
    val vehicleUuid: String,
    val prefix: String,
    val latitude: Double,
    val longitude: Double,
    /** The fix is valid and recent; a stale one is still sent so the display can grey it out. */
    val live: Boolean = false,
    val reportedAt: String? = null,
    val ignition: Boolean? = null,
)

@Serializable
data class GarageMap(
    val baseLatitude: Double? = null,
    val baseLongitude: Double? = null,
    val baseRadiusMeters: Double = 0.0,
    val vehicles: List<GarageMapVehicle> = emptyList(),
)
