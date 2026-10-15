@file:OptIn(kotlinx.serialization.ExperimentalSerializationApi::class)

package com.progressrail.hermes.client.shared

import kotlinx.serialization.EncodeDefault
import kotlinx.serialization.ExperimentalSerializationApi
import kotlinx.serialization.Serializable

/** `GET` list envelope of the platform's paged resources (`PageJson`). */
@Serializable
data class Page<T>(
    val items: List<T> = emptyList(),
    val page: Long = 0,
    val pageSize: Long = 0,
    val totalItems: Long = 0,
    val totalPages: Long = 0,
)

/** `GET /vehicle`: for a driver, only the vehicle(s) assigned to them (`HRMS-860`). */
@Serializable
data class Vehicle(
    val uuid: String,
    val plate: String,
    val model: String = "",
    val status: String = "",
    /** The fleet prefix a driver confirms when reporting a fuelling (`TRM-536`). */
    val prefix: String? = null,
    /** The official reading, kept by the server. */
    val odometerKm: Double? = null,
)

@Serializable
data class ChecklistTemplateItem(
    val uuid: String,
    val description: String,
    val generatesWorkOrder: Boolean = false,
)

/** `GET /checklist-template` rows carry no items; read one by uuid for them. */
@Serializable
data class ChecklistTemplate(
    val uuid: String,
    val name: String,
    val checklistType: String,
    val active: Boolean = true,
    val isDefault: Boolean = false,
    val items: List<ChecklistTemplateItem> = emptyList(),
)

@Serializable
data class ChecklistAnswer(
    val checklistTemplateItemUuid: String,
    /** [ANSWER_CONFORMING] or [ANSWER_NON_CONFORMING]. */
    val status: String,
    val observation: String? = null,
)

/** Request and response of `POST /checklist-run`, and the shape `GET .../possession` returns. */
@Serializable
data class ChecklistRun(
    val uuid: String? = null,
    val checklistTemplateUuid: String = "",
    val driverUuid: String = "",
    val vehicleUuid: String = "",
    val checklistType: String,
    @EncodeDefault val odometerKm: Double = 0.0,
    val notes: String? = null,
    val openingChecklistUuid: String? = null,
    val answers: List<ChecklistAnswer> = emptyList(),
)

/** The platform's error body (`ErrorResponseJson`): a stable key and a localized message. */
@Serializable
data class ApiError(
    val errorKey: String = "",
    val message: String = "",
)

@Serializable
data class GeoPoint(val latitude: Double, val longitude: Double)

/** `GET /vehicle/uuid/{uuid}/tracking`: read live, never stored (`HRMS-928`..`930`). Instants are ISO-8601 in UTC. */
@Serializable
data class VehicleTracking(
    val ignition: String = "Unknown",
    val source: String = "",
    val reportedAt: String? = null,
    val trustworthy: Boolean = false,
    val position: GeoPoint? = null,
    val positionReportedAt: String? = null,
    /** Whether the coordinate shows where the vehicle is now; a stale one must never read as live. */
    val positionLive: Boolean = false,
    val odometerMeters: Double? = null,
)

/** `POST /fuel-entry/receipt` body (`TRM-520/542`); the driver is the caller, so no driver uuid is sent. */
@Serializable
data class FuelReceiptRequest(
    val vehicleUuid: String,
    val vehiclePrefix: String,
    /** UTC, no zone (`NaiveDateTime`). */
    val recordedAt: String,
    val volumeLiters: Double,
    val valueCents: Long,
    val odometerKm: Double? = null,
    val station: String? = null,
    // The server requires both flags; with the default JSON setting a `false` default would be left out (422).
    @EncodeDefault val fullTank: Boolean = false,
    @EncodeDefault val overrideOdometer: Boolean = false,
)

@Serializable
data class FuelEntry(
    val uuid: String? = null,
    val vehicleUuid: String = "",
    val recordedAt: String = "",
    val volumeLiters: Double = 0.0,
    val valueCents: Long = 0,
    val station: String? = null,
    val fullTank: Boolean = false,
)

/** What the server answered to a receipt: `201` created it, `200` found the same fuelling already recorded (`TRM-541`). */
data class FuelReceiptResult(val entry: FuelEntry, val alreadyRecorded: Boolean)

/** `GET /extra-trip` row; dates are `yyyy-MM-dd`, times `HH:mm:ss`. */
@Serializable
data class ExtraTrip(
    val uuid: String = "",
    val orderCode: String = "",
    val tripDate: String = "",
    val startTime: String? = null,
    val returnDate: String? = null,
    val returnTime: String? = null,
    val originCity: String? = null,
    val destination: String? = null,
    val stops: String? = null,
    val status: String = "",
)

const val CHECKLIST_DEPARTURE = "Departure"
const val CHECKLIST_RETURN = "Return"
const val ANSWER_CONFORMING = "Conforming"
const val ANSWER_NON_CONFORMING = "NonConforming"
