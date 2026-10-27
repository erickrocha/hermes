package com.erocha.hermes.client.tv

import com.erocha.hermes.client.shared.GarageDeviceApi
import com.erocha.hermes.client.shared.PairGarageDeviceRequest
import io.ktor.client.HttpClient
import io.ktor.client.engine.mock.MockEngine
import io.ktor.client.engine.mock.respond
import io.ktor.client.plugins.contentnegotiation.ContentNegotiation
import io.ktor.http.ContentType
import io.ktor.http.HttpHeaders
import io.ktor.http.HttpMethod
import io.ktor.http.HttpStatusCode
import io.ktor.http.headersOf
import io.ktor.serialization.kotlinx.json.json
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Test

class GarageDeviceApiTest {
    @Test
    fun pairUsesPublicEndpointAndReadsDeviceCredential() = runTest {
        val engine = MockEngine { request ->
            assertEquals(HttpMethod.Post, request.method)
            assertEquals("/garage-device/pair", request.url.encodedPath)
            respond(
                content = """
                    {
                      "accessToken": "device-token",
                      "tokenType": "Bearer",
                      "expiresAt": 1792000000,
                      "deviceUuid": "device-uuid",
                      "deviceName": "Garage display"
                    }
                """.trimIndent(),
                status = HttpStatusCode.Created,
                headers = headersOf(HttpHeaders.ContentType, ContentType.Application.Json.toString()),
            )
        }
        val client = HttpClient(engine) {
            install(ContentNegotiation) {
                json(Json { ignoreUnknownKeys = true })
            }
        }

        val credential = GarageDeviceApi(client, "https://api.example.test/").pair("pairing-code")

        assertEquals("device-token", credential.accessToken)
        assertEquals("device-uuid", credential.deviceUuid)
        assertEquals("Garage display", credential.deviceName)
        assertEquals(
            """{"code":"pairing-code"}""",
            Json.encodeToString(PairGarageDeviceRequest("pairing-code")),
        )
        client.close()
    }

    @Test
    fun deviceCredentialCanOnlyCallTheReadOnlyMonitorRoutes() = runTest {
        val expectedPaths = listOf(
            "/garage-attendance/monitor",
            "/garage-attendance/monitor/cards",
            "/garage-attendance/call-list",
            "/garage-attendance/monitor/matrix",
            "/garage-attendance/monitor/map",
        )
        var requestIndex = 0
        val engine = MockEngine { request ->
            assertEquals(HttpMethod.Get, request.method)
            val path = expectedPaths[requestIndex++]
            assertEquals(path, request.url.encodedPath)
            assertEquals("Bearer device-token", request.headers[HttpHeaders.Authorization])
            respond(
                content = when (path) {
                    "/garage-attendance/monitor" -> """
                        [{"vehicleUuid":"vehicle-1","prefix":"ABC-1234","requiredPending":["oil"],"fillTankAlert":true,"calledByManager":false,"readiness":"Alert","urgent":true}]
                    """.trimIndent()
                    "/garage-attendance/monitor/cards" -> """
                        {"cards":[{"vehicleUuid":"vehicle-1","prefix":"ABC-1234","tankPercent":18.5,"needsRefuel":true,"maintenanceStatus":"Overdue","lastGarageCycleDistanceKm":12450.0}],"notShown":2}
                    """.trimIndent()
                    "/garage-attendance/call-list" -> """
                        [{"vehicleUuid":"vehicle-2","prefix":"DEF-5678","nextDeparture":"2026-10-15T12:00:00","calledByManager":true,"hasDepartureAhead":true,"tankCalls":false}]
                    """.trimIndent()
                    "/garage-attendance/monitor/map" -> """
                        {"baseLatitude":-23.5,"baseLongitude":-46.6,"baseRadiusMeters":150.0,"vehicles":[{"vehicleUuid":"vehicle-1","prefix":"ABC-1234","latitude":-23.6,"longitude":-46.7,"live":true,"reportedAt":"2026-10-27T10:00:00+00:00","ignition":true}]}
                    """.trimIndent()
                    else -> """
                        {"columns":[{"serviceModelUuid":"service-1","name":"Oil","internal":false}],"rows":[{"vehicleUuid":"vehicle-1","prefix":"ABC-1234","cells":[{"effectiveState":"pending","stamp":"2026-10-10T08:00:00","lastPerformedAt":"2026-09-01T08:00:00"}]}],"rowsNotShown":1,"columnsNotShown":0}
                    """.trimIndent()
                },
                status = HttpStatusCode.OK,
                headers = headersOf(HttpHeaders.ContentType, ContentType.Application.Json.toString()),
            )
        }
        val client = HttpClient(engine) {
            install(ContentNegotiation) {
                json(Json { ignoreUnknownKeys = true })
            }
        }
        val api = GarageDeviceApi(client, "https://api.example.test")

        val monitor = api.monitor("device-token")
        val cards = api.monitorCards("device-token")
        val callList = api.callList("device-token")
        val matrix = api.monitorMatrix("device-token")
        val map = api.monitorMap("device-token")

        assertEquals(expectedPaths.size, requestIndex)
        assertEquals("ABC-1234", monitor.single().prefix)
        assertEquals("Alert", monitor.single().readiness)
        assertEquals(true, monitor.single().urgent)
        assertEquals(18.5, cards.cards.single().tankPercent)
        assertEquals(2, cards.notShown)
        assertEquals(true, callList.single().calledByManager)
        assertEquals("Oil", matrix.columns.single().name)
        assertEquals("pending", matrix.rows.single().cells.single()?.effectiveState)
        assertEquals("2026-10-10T08:00:00", matrix.rows.single().cells.single()?.stamp)
        assertEquals(1, matrix.rowsNotShown)
        assertEquals(-23.5, map.baseLatitude)
        assertEquals(true, map.vehicles.single().live)
        client.close()
    }
}