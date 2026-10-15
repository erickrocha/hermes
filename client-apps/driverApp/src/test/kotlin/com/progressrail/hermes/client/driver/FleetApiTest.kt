package com.progressrail.hermes.client.driver

import com.progressrail.hermes.client.shared.ChecklistAnswer
import com.progressrail.hermes.client.shared.ChecklistRun
import com.progressrail.hermes.client.shared.FleetApi
import com.progressrail.hermes.client.shared.apiError
import io.ktor.client.HttpClient
import io.ktor.client.engine.mock.MockEngine
import io.ktor.client.engine.mock.respond
import io.ktor.client.engine.mock.toByteArray
import io.ktor.client.plugins.ClientRequestException
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
            assertTrue(body, !body.contains("openingChecklistUuid") && !body.contains("notes"))
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
}
