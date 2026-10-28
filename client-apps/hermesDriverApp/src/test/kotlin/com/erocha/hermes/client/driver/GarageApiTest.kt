package com.erocha.hermes.client.driver

import com.erocha.hermes.client.shared.GarageApi
import com.erocha.hermes.client.shared.STATE_PERFORMED
import io.ktor.client.HttpClient
import io.ktor.client.engine.mock.MockEngine
import io.ktor.client.engine.mock.respond
import io.ktor.client.engine.mock.toByteArray
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
import org.junit.Assert.assertTrue
import org.junit.Test

class GarageApiTest {
    private val json = headersOf(HttpHeaders.ContentType, ContentType.Application.Json.toString())

    private fun client(engine: MockEngine) = HttpClient(engine) {
        expectSuccess = true
        install(ContentNegotiation) { json(Json { ignoreUnknownKeys = true }) }
    }

    private val attendance = """{"uuid":"a1","vehicleUuid":"v1","status":"Open","requiredPending":["Diesel"],"services":[
        {"uuid":"s1","name":"Diesel","serviceGroup":"External","requiredForDeparture":true,"state":"Pending","effectiveState":"Pending","effectiveReason":"Pending"},
        {"uuid":"s2","name":"WC","serviceGroup":"Internal","requiredForDeparture":false,"state":"Performed","effectiveState":"Performed","effectiveReason":"Valid","performedAt":"2026-10-27T10:00:00"}]}"""

    @Test
    fun theQueueIsReadWithTheBearerToken() = runTest {
        val engine = MockEngine { request ->
            assertEquals("Bearer tok", request.headers[HttpHeaders.Authorization])
            assertEquals("/garage-attendance/queue", request.url.encodedPath)
            respond("""[{"attendanceUuid":"a1","vehicleUuid":"v1","prefix":"2424","requiredPending":["Diesel"],"fillTankAlert":true,"calledByManager":false}]""", HttpStatusCode.OK, json)
        }
        val queue = GarageApi(client(engine), "https://api.test/").queue("tok")
        assertEquals("2424", queue.single().prefix)
        assertEquals(listOf("Diesel"), queue.single().requiredPending)
        assertTrue(queue.single().fillTankAlert)
    }

    @Test
    fun markingAServiceSendsTheStateAndRereadsTheTriage() = runTest {
        val seen = mutableListOf<String>()
        val engine = MockEngine { request ->
            seen += "${request.method.value} ${request.url.encodedPath}"
            if (request.method == HttpMethod.Put) {
                assertEquals("""{"state":"Performed"}""", String(request.body.toByteArray()))
                respond("""{"uuid":"s1","name":"Diesel","serviceGroup":"External","state":"Performed","effectiveState":"Performed"}""", HttpStatusCode.OK, json)
            } else {
                respond(attendance, HttpStatusCode.OK, json)
            }
        }
        val result = GarageApi(client(engine), "https://api.test").mark("tok", "a1", "s1", STATE_PERFORMED)
        assertEquals(listOf("PUT /garage-attendance/uuid/a1/service/s1", "GET /garage-attendance/uuid/a1"), seen)
        assertEquals(listOf("External", "Internal"), result.services.map { it.serviceGroup })
        assertEquals("2026-10-27T10:00:00", result.services[1].performedAt)
    }

    @Test
    fun openingATriagePostsTheVehicle() = runTest {
        val engine = MockEngine { request ->
            assertEquals(HttpMethod.Post, request.method)
            assertEquals("""{"vehicleUuid":"v1"}""", String(request.body.toByteArray()))
            respond(attendance, HttpStatusCode.Created, json)
        }
        assertEquals("a1", GarageApi(client(engine), "https://api.test").open("tok", "v1").uuid)
    }
}
