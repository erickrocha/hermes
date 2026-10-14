package com.progressrail.hermes.client.shared

import io.ktor.client.HttpClient
import io.ktor.client.call.body
import io.ktor.client.request.bearerAuth
import io.ktor.client.request.get
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.contentType
import kotlinx.serialization.Serializable

@Serializable
data class PairGarageDeviceRequest(
    val code: String,
)

@Serializable
data class GarageDeviceCredential(
    val accessToken: String,
    val tokenType: String,
    val expiresAt: Long,
    val deviceUuid: String,
    val deviceName: String,
)

class GarageDeviceApi(
    private val httpClient: HttpClient,
    private val baseUrl: String,
) {
    suspend fun pair(code: String): GarageDeviceCredential =
        httpClient.post("${baseUrl.trimEnd('/')}/garage-device/pair") {
            contentType(ContentType.Application.Json)
            setBody(PairGarageDeviceRequest(code))
        }.body()

    suspend fun monitor(accessToken: String): List<GarageMonitorEntry> =
        getReadOnly("/garage-attendance/monitor", accessToken)

    suspend fun monitorCards(accessToken: String): GarageMonitorCards =
        getReadOnly("/garage-attendance/monitor/cards", accessToken)

    suspend fun callList(accessToken: String): List<GarageCallListEntry> =
        getReadOnly("/garage-attendance/call-list", accessToken)

    suspend fun monitorMatrix(accessToken: String): GarageMatrix =
        getReadOnly("/garage-attendance/monitor/matrix", accessToken)

    private suspend inline fun <reified T> getReadOnly(path: String, accessToken: String): T =
        httpClient.get("${baseUrl.trimEnd('/')}$path") {
            bearerAuth(accessToken)
        }.body()
}