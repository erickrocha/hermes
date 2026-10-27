package com.erocha.hermes.client.shared

import io.ktor.client.HttpClient
import io.ktor.client.call.body
import io.ktor.client.request.bearerAuth
import io.ktor.client.request.get
import io.ktor.client.request.post
import io.ktor.client.request.put
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.contentType
import kotlinx.serialization.Serializable

/** One vehicle of the yard's queue (`GET /garage-attendance/queue`). */
@Serializable
data class GarageQueueEntry(
    val attendanceUuid: String? = null,
    val vehicleUuid: String,
    val prefix: String,
    val manualPriority: Int? = null,
    val nextDeparture: String? = null,
    val requiredPending: List<String> = emptyList(),
    val fillTankAlert: Boolean = false,
    val calledByManager: Boolean = false,
)

/** A service record of a triage; [effectiveState] is what the state is now, [state] what was last marked. */
@Serializable
data class GarageService(
    val uuid: String? = null,
    val name: String,
    /** `External` or `Internal` (the access profile gates each group). */
    val serviceGroup: String,
    val requiredForDeparture: Boolean = false,
    val state: String = STATE_PENDING,
    val effectiveState: String = STATE_PENDING,
    val performedAt: String? = null,
    val markedAt: String? = null,
)

@Serializable
data class GarageAttendance(
    val uuid: String? = null,
    val vehicleUuid: String,
    val status: String = "Open",
    val requiredPending: List<String> = emptyList(),
    val services: List<GarageService> = emptyList(),
)

@Serializable
private data class ServiceMark(val state: String)

@Serializable
private data class AttendanceOpen(val vehicleUuid: String)

const val STATE_PENDING = "Pending"
const val STATE_PERFORMED = "Performed"
const val STATE_NOT_NEEDED = "NotNeeded"
const val STATE_NOT_DONE = "NotDone"

/**
 * The garage-yard routes the mechanic uses on the phone (`HRMS-958`/`959`). It only transports and decodes: who may
 * mark which group of services is the server's call (the access profile, `PermissionNotInProfile`).
 */
class GarageApi(
    private val httpClient: HttpClient,
    private val baseUrl: String,
) {
    private fun url(path: String) = "${baseUrl.trimEnd('/')}$path"

    suspend fun queue(accessToken: String): List<GarageQueueEntry> =
        httpClient.get(url("/garage-attendance/queue")) { bearerAuth(accessToken) }.body()

    suspend fun attendance(accessToken: String, uuid: String): GarageAttendance =
        httpClient.get(url("/garage-attendance/uuid/$uuid")) { bearerAuth(accessToken) }.body()

    /** Opens a triage for a vehicle that has none; the server answers 409 when one is already active. */
    suspend fun open(accessToken: String, vehicleUuid: String): GarageAttendance =
        httpClient.post(url("/garage-attendance")) {
            bearerAuth(accessToken)
            contentType(ContentType.Application.Json)
            setBody(AttendanceOpen(vehicleUuid))
        }.body()

    /** Marks one service; the answer is the triage as it reads now (effective states recomputed). */
    suspend fun mark(accessToken: String, attendanceUuid: String, serviceUuid: String, state: String): GarageAttendance {
        httpClient.put(url("/garage-attendance/uuid/$attendanceUuid/service/$serviceUuid")) {
            bearerAuth(accessToken)
            contentType(ContentType.Application.Json)
            setBody(ServiceMark(state))
        }
        return attendance(accessToken, attendanceUuid)
    }
}
