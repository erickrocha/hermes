package com.progressrail.hermes.client.shared

import io.ktor.client.HttpClient
import io.ktor.client.call.body
import io.ktor.client.plugins.ClientRequestException
import io.ktor.client.request.bearerAuth
import io.ktor.client.request.get
import io.ktor.client.request.parameter
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.contentType

/**
 * The platform's public fleet-operations routes the driver app consumes (`HRMS-504`). It only transports and
 * decodes: scope, kilometre and possession rules are the server's (`SR-CA-03-01`).
 */
class FleetApi(
    private val httpClient: HttpClient,
    private val baseUrl: String,
) {
    private fun url(path: String) = "${baseUrl.trimEnd('/')}$path"

    suspend fun vehicles(accessToken: String): List<Vehicle> = allPages { page ->
        httpClient.get(url("/vehicle")) {
            bearerAuth(accessToken)
            parameter("page", page)
            parameter("pageSize", PAGE_SIZE)
        }.body<Page<Vehicle>>()
    }

    /** The checklist currently holding the vehicle, or null when nobody does. */
    suspend fun possession(accessToken: String, vehicleUuid: String): ChecklistRun? =
        httpClient.get(url("/vehicle/uuid/$vehicleUuid/possession")) { bearerAuth(accessToken) }.body()

    suspend fun templates(accessToken: String): List<ChecklistTemplate> = allPages { page ->
        httpClient.get(url("/checklist-template")) {
            bearerAuth(accessToken)
            parameter("page", page)
            parameter("pageSize", PAGE_SIZE)
        }.body<Page<ChecklistTemplate>>()
    }

    suspend fun template(accessToken: String, uuid: String): ChecklistTemplate =
        httpClient.get(url("/checklist-template/uuid/$uuid")) { bearerAuth(accessToken) }.body()

    suspend fun submitRun(accessToken: String, run: ChecklistRun): ChecklistRun =
        httpClient.post(url("/checklist-run")) {
            bearerAuth(accessToken)
            contentType(ContentType.Application.Json)
            setBody(run)
        }.body()

    private suspend fun <T> allPages(fetch: suspend (Long) -> Page<T>): List<T> {
        val all = mutableListOf<T>()
        var page = 0L
        while (true) {
            val current = fetch(page)
            all += current.items
            page += 1
            if (page >= current.totalPages || current.items.isEmpty()) return all
        }
    }

    private companion object {
        const val PAGE_SIZE = 50
    }
}

/** The server's reason for a refused call, or null when the body is not the platform's error shape. */
suspend fun ClientRequestException.apiError(): ApiError? =
    runCatching { response.body<ApiError>() }.getOrNull()?.takeIf { it.message.isNotBlank() }
