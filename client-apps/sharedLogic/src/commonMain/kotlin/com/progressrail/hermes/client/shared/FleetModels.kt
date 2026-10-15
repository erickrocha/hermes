package com.progressrail.hermes.client.shared

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
    val odometerKm: Double = 0.0,
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

const val CHECKLIST_DEPARTURE = "Departure"
const val CHECKLIST_RETURN = "Return"
const val ANSWER_CONFORMING = "Conforming"
const val ANSWER_NON_CONFORMING = "NonConforming"
