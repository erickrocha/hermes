package com.erocha.hermes.client.driver

import com.erocha.hermes.client.shared.ChecklistAnswer
import com.erocha.hermes.client.shared.ChecklistRun
import com.erocha.hermes.client.shared.FleetApi
import com.erocha.hermes.client.shared.FuelReceiptRequest
import com.erocha.hermes.client.shared.apiError
import io.ktor.client.HttpClient
import io.ktor.client.engine.mock.MockEngine
import io.ktor.client.engine.mock.respond
import io.ktor.client.engine.mock.toByteArray
import io.ktor.client.plugins.ClientRequestException
import io.ktor.client.plugins.ServerResponseException
import io.ktor.client.plugins.contentnegotiation.ContentNegotiation
import io.ktor.http.ContentType
import io.ktor.http.HttpHeaders
import io.ktor.http.HttpMethod
import io.ktor.http.HttpStatusCode
import io.ktor.http.headersOf
import io.ktor.serialization.kotlinx.json.json
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.Json
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

class FleetApiTest {
    private val json = headersOf(HttpHeaders.ContentType, ContentType.Application.Json.toString())

    private fun client(engine: MockEngine) = HttpClient(engine) {
        expectSuccess = true
        install(ContentNegotiation) { json(Json { ignoreUnknownKeys = true }) }
    }

    @Test
    fun vehiclesAreReadPageByPageWithTheBearerToken() = runTest {
        val seen = mutableListOf<String>()
        val engine = MockEngine { request ->
            assertEquals("Bearer tok", request.headers[HttpHeaders.Authorization])
            seen += request.url.parameters["page"]!!
            val page = request.url.parameters["page"]!!.toInt()
            respond(
                """{"items":[{"uuid":"v$page","plate":"AAA000$page","model":"Bus","status":"Active","odometerKm":1100.0}],"page":$page,"pageSize":50,"totalItems":2,"totalPages":2}""",
                HttpStatusCode.OK, json,
            )
        }
        val vehicles = FleetApi(client(engine), "https://api.test/").vehicles("tok")
        assertEquals(listOf("0", "1"), seen)
        assertEquals(listOf("v0", "v1"), vehicles.map { it.uuid })
        assertEquals(1100.0, vehicles[0].odometerKm!!, 0.0)
    }

    @Test
    fun noHolderIsANullPossession() = runTest {
        val engine = MockEngine { request ->
            assertEquals("/vehicle/uuid/v1/possession", request.url.encodedPath)
            respond("null", HttpStatusCode.OK, json)
        }
        assertNull(FleetApi(client(engine), "https://api.test").possession("tok", "v1"))
    }

    @Test
    fun theRunIsPostedAsJsonWithoutEmptyOptionalFields() = runTest {
        val engine = MockEngine { request ->
            assertEquals(HttpMethod.Post, request.method)
            assertEquals("/checklist-run", request.url.encodedPath)
            val body = request.body.toByteArray().decodeToString()
            assertTrue(body, body.contains("\"driverUuid\":\"me\"") && body.contains("\"checklistType\":\"Departure\""))
            assertTrue(body, !body.contains("openingChecklistUuid") && !body.contains("notes") && body.contains("\"odometerKm\":10.0"))
            respond("""{"uuid":"run-9","checklistType":"Departure","answers":[]}""", HttpStatusCode.Created, json)
        }
        val saved = FleetApi(client(engine), "https://api.test").submitRun(
            "tok",
            ChecklistRun(checklistTemplateUuid = "t", driverUuid = "me", vehicleUuid = "v", checklistType = "Departure", odometerKm = 10.0,
                answers = listOf(ChecklistAnswer("i1", "Conforming"))),
        )
        assertEquals("run-9", saved.uuid)
    }

    @Test
    fun aRefusalCarriesTheServersReason() = runTest {
        val engine = MockEngine {
            respond("""{"errorKey":"OdometerBelowOfficial","message":"A quilometragem é menor que a leitura oficial"}""", HttpStatusCode.BadRequest, json)
        }
        try {
            FleetApi(client(engine), "https://api.test").submitRun("tok", ChecklistRun(checklistType = "Departure"))
            fail("expected a refusal")
        } catch (failure: ClientRequestException) {
            assertEquals(400, failure.response.status.value)
            assertEquals("OdometerBelowOfficial", failure.apiError()?.errorKey)
            assertEquals("A quilometragem é menor que a leitura oficial", failure.apiError()?.message)
        }
    }

    @Test
    fun aBodyThatIsNotTheErrorShapeGivesNoReason() = runTest {
        val engine = MockEngine { respond("oops", HttpStatusCode.BadGateway, headersOf(HttpHeaders.ContentType, "text/plain")) }
        try {
            FleetApi(client(engine), "https://api.test").vehicles("tok")
            fail("expected a failure")
        } catch (failure: ClientRequestException) {
            assertNull(failure.apiError())
        } catch (_: Exception) {
            // a 5xx raises ServerResponseException, which carries no reason either
        }
    }

    @Test
    fun trackingKeepsTheLiveFlagAndTheInstants() = runTest {
        val engine = MockEngine { request ->
            assertEquals("/vehicle/uuid/v1/tracking", request.url.encodedPath)
            respond(
                """{"deviceId":9,"ignition":"On","source":"LastPosition","reportedAt":"2026-10-15T12:00:00Z","trustworthy":true,
                    "position":{"latitude":-23.5,"longitude":-46.6},"positionReportedAt":"2026-10-15T11:00:00Z","positionLive":false}""",
                HttpStatusCode.OK, json,
            )
        }
        val tracking = FleetApi(client(engine), "https://api.test").tracking("tok", "v1")
        assertEquals(false, tracking.positionLive)
        assertEquals(-23.5, tracking.position!!.latitude, 0.0)
        assertEquals("2026-10-15T11:00:00Z", tracking.positionReportedAt)
        assertEquals("On", tracking.ignition)
    }

    @Test
    fun aReceiptIsCreatedOrFoundAlreadyRecordedAccordingToTheStatus() = runTest {
        val body = """{"uuid":"f1","vehicleUuid":"v1","recordedAt":"2026-10-15T12:00:00","volumeLiters":40.0,"valueCents":30000,"fullTank":true}"""
        val receipt = FuelReceiptRequest("v1", "P-12", "2026-10-15T12:00:00", 40.0, 30000L, fullTank = true)
        val created = MockEngine { request ->
            val sent = request.body.toByteArray().decodeToString()
            assertTrue(sent, sent.contains("\"vehiclePrefix\":\"P-12\"") && !sent.contains("driverUuid"))
            // The server requires both flags, so they must be sent even when false.
            assertTrue(sent, sent.contains("\"fullTank\":true") && sent.contains("\"overrideOdometer\":false"))
            respond(body, HttpStatusCode.Created, json)
        }
        val first = FleetApi(client(created), "https://api.test").submitReceipt("tok", receipt)
        assertEquals(false, first.alreadyRecorded)
        assertEquals("f1", first.entry.uuid)
        val duplicate = MockEngine { respond(body, HttpStatusCode.OK, json) }
        assertEquals(true, FleetApi(client(duplicate), "https://api.test").submitReceipt("tok", receipt).alreadyRecorded)
    }

    @Test
    fun theOdometerWindowRefusalKeepsItsKey() = runTest {
        val engine = MockEngine {
            respond("""{"errorKey":"OdometerOutsideWindow","message":"fora da janela"}""", HttpStatusCode.Conflict, json)
        }
        try {
            FleetApi(client(engine), "https://api.test").submitReceipt("tok", FuelReceiptRequest("v1", "P", "t", 1.0, 1L))
            fail("expected a refusal")
        } catch (failure: ClientRequestException) {
            assertEquals(409, failure.response.status.value)
            assertEquals("OdometerOutsideWindow", failure.apiError()?.errorKey)
        }
    }

    @Test
    fun ownFuellingsAndTripsAreReadFromTheirPagedRoutes() = runTest {
        val engine = MockEngine { request ->
            assertEquals("20", request.url.parameters["pageSize"])
            when (request.url.encodedPath) {
                "/fuel-entry" -> respond("""{"items":[{"uuid":"f1","recordedAt":"2026-10-15T12:00:00","volumeLiters":40.0,"valueCents":30000}],"page":0,"pageSize":20,"totalItems":1,"totalPages":1}""", HttpStatusCode.OK, json)
                else -> respond("""{"items":[{"uuid":"t1","orderCode":"QA-1","tripDate":"2026-11-20","startTime":"08:00:00","destination":"Porto","status":"Scheduled"}],"page":0,"pageSize":20,"totalItems":1,"totalPages":1}""", HttpStatusCode.OK, json)
            }
        }
        val api = FleetApi(client(engine), "https://api.test")
        assertEquals(30000L, api.fuelEntries("tok").single().valueCents)
        val trip = api.trips("tok").single()
        assertEquals("QA-1", trip.orderCode)
        assertEquals("08:00:00", trip.startTime)
    }

    @Test
    fun aProviderOutageIsAServerErrorThatStillCarriesItsReason() = runTest {
        val engine = MockEngine {
            respond("""{"errorKey":"TrackingUnavailable","message":"indisponível"}""", HttpStatusCode.ServiceUnavailable, json)
        }
        try {
            FleetApi(client(engine), "https://api.test").tracking("tok", "v1")
            fail("expected a failure")
        } catch (failure: ServerResponseException) {
            assertEquals(503, failure.response.status.value)
            assertEquals("TrackingUnavailable", failure.apiError()?.errorKey)
        }
    }
}
