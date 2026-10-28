package com.erocha.hermes.client.driver

import com.erocha.hermes.client.shared.InspectionAnswer
import com.erocha.hermes.client.shared.TechnicalInspectionRequest
import com.erocha.hermes.client.shared.WorkshopApi
import com.erocha.hermes.client.shared.isOpen
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
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class WorkshopApiTest {
    private val json = headersOf(HttpHeaders.ContentType, ContentType.Application.Json.toString())

    private fun client(engine: MockEngine) = HttpClient(engine) {
        expectSuccess = true
        install(ContentNegotiation) { json(Json { ignoreUnknownKeys = true }) }
    }

    private val order = """{"uuid":"o1","number":"OS-1","vehicleUuid":"v1","description":"Freios","status":"Open","items":[{"uuid":"i1","description":"Pastilha","status":"Pending"}]}"""

    @Test
    fun onlyActiveInspectionModelsAreOffered() = runTest {
        val engine = MockEngine {
            respond(
                """{"items":[{"name":"Valeta","items":["Pneus","Luzes"],"active":true},{"name":"Antigo","items":["X"],"active":false}],"page":0,"pageSize":50,"totalItems":2,"totalPages":1}""",
                HttpStatusCode.OK, json,
            )
        }
        val models = WorkshopApi(client(engine), "https://api.test").inspectionModels("tok")
        assertEquals(listOf("Valeta"), models.map { it.name })
        assertEquals(listOf("Pneus", "Luzes"), models.single().items)
    }

    @Test
    fun anInspectionIsPostedWithItsAnswersAndReportsTheWorkOrderItOpened() = runTest {
        val engine = MockEngine { request ->
            assertEquals("/technical-inspection", request.url.encodedPath)
            val body = String(request.body.toByteArray())
            assertTrue(body, body.contains(""""vehicleUuid":"v1"""") && body.contains(""""conforming":false"""))
            respond("""{"uuid":"t1","workOrderOpened":true,"workOrderUuids":["o1"]}""", HttpStatusCode.Created, json)
        }
        val request = TechnicalInspectionRequest("v1", "Valeta", items = listOf(InspectionAnswer("Pneus", false, "careca")))
        val result = WorkshopApi(client(engine), "https://api.test").submitInspection("tok", request)
        assertTrue(result.workOrderOpened)
        assertEquals(listOf("o1"), result.workOrderUuids)
    }

    @Test
    fun addingAnItemAndIssuingAPartBothReturnTheOrderAsItReadsNow() = runTest {
        val seen = mutableListOf<String>()
        val engine = MockEngine { request ->
            seen += "${request.method.value} ${request.url.encodedPath}"
            if (request.method == HttpMethod.Get) respond(order, HttpStatusCode.OK, json)
            else respond("{}", HttpStatusCode.Created, json)
        }
        val api = WorkshopApi(client(engine), "https://api.test")
        api.addItem("tok", "o1", "Trocar pastilha")
        val after = api.issuePart("tok", "o1", "p1", 2.0, "i1")
        assertEquals(
            listOf(
                "POST /work-order/uuid/o1/item", "GET /work-order/uuid/o1",
                "POST /work-order/uuid/o1/issue-part", "GET /work-order/uuid/o1",
            ),
            seen,
        )
        assertEquals("OS-1", after.number)
    }

    @Test
    fun resolvingAnItemPutsWhatWasDoneAndRereadsTheOrder() = runTest {
        val seen = mutableListOf<String>()
        val engine = MockEngine { request ->
            seen += "${request.method.value} ${request.url.encodedPath}"
            if (request.method == HttpMethod.Put) {
                assertEquals("""{"resolutionDescription":"Trocada"}""", String(request.body.toByteArray()))
                respond("", HttpStatusCode.NoContent)
            } else {
                respond(order, HttpStatusCode.OK, json)
            }
        }
        WorkshopApi(client(engine), "https://api.test").resolveItem("tok", "o1", "i1", "Trocada")
        assertEquals(listOf("PUT /work-order/uuid/o1/item/uuid/i1/resolve", "GET /work-order/uuid/o1"), seen)
    }

    @Test
    fun concludedAndCancelledOrdersAreNotOpen() {
        assertTrue(WorkshopApiTest().orderWith("Open").isOpen())
        assertFalse(orderWith("Concluded").isOpen())
        assertFalse(orderWith("Cancelled").isOpen())
    }

    private fun orderWith(status: String) = com.erocha.hermes.client.shared.WorkOrder(vehicleUuid = "v1", status = status)
}
