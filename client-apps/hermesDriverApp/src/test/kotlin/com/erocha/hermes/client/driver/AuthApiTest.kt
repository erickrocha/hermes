package com.erocha.hermes.client.driver

import com.erocha.hermes.client.shared.AuthApi
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

class AuthApiTest {
    private val json = headersOf(HttpHeaders.ContentType, ContentType.Application.Json.toString())

    private fun client(engine: MockEngine) = HttpClient(engine) {
        // Same behaviour as createHermesHttpClient: a 4xx must raise so the activity can react to 401.
        expectSuccess = true
        install(ContentNegotiation) { json(Json { ignoreUnknownKeys = true }) }
    }

    private val token = """
        {"accessToken":"access-1","tokenType":"Bearer","expireIn":1792000000,"refreshToken":"refresh-1",
         "email":"d@x.test","uuid":"u","name":"Maria","userId":7,"scope":"Tenant","profileKey":"Driver","profileName":"Motorista",
         "permissions":["driver.act","vehicle.read@own","checklist-run.create@own"],"unknownField":1}
    """.trimIndent()

    @Test
    fun loginPostsAFormAndReadsTheSession() = runTest {
        val engine = MockEngine { request ->
            assertEquals(HttpMethod.Post, request.method)
            assertEquals("/login", request.url.encodedPath)
            val body = request.body.toByteArray().decodeToString()
            assertTrue(body, body.contains("email=d%40x.test") && body.contains("password=s3cret"))
            assertTrue(request.body.contentType.toString().startsWith("application/x-www-form-urlencoded"))
            respond(token, HttpStatusCode.OK, json)
        }
        val response = AuthApi(client(engine), "https://api.example.test/").login("d@x.test", "s3cret")
        assertEquals("access-1", response.accessToken)
        assertEquals("refresh-1", response.refreshToken)
        assertEquals(1792000000L, response.expireIn)
        assertEquals("Tenant", response.scope)
        assertEquals("Driver", response.profileKey)
        assertEquals("Motorista", response.profileName)
        assertEquals(listOf("driver.act", "vehicle.read@own", "checklist-run.create@own"), response.permissions)
        assertNull(response.tenantId)
        assertNull(response.tenantUuid)
    }

    @Test
    fun aResponseWithoutProfileFieldsStillReadsAndCarriesNoGrants() = runTest {
        val minimal = """{"accessToken":"access-1","expireIn":1792000000,"refreshToken":"refresh-1","name":"Maria"}"""
        val engine = MockEngine { respond(minimal, HttpStatusCode.OK, json) }
        val response = AuthApi(client(engine), "https://api.example.test").login("d@x.test", "s3cret")
        assertEquals("Tenant", response.scope)
        assertNull(response.profileKey)
        assertEquals(emptyList<String>(), response.permissions)
    }

    @Test
    fun badCredentialsRaiseUnauthorized() = runTest {
        val engine = MockEngine { respond("""{"errorKey":"BadCredentials"}""", HttpStatusCode.Unauthorized, json) }
        try {
            AuthApi(client(engine), "https://api.example.test").login("d@x.test", "wrong")
            fail("expected 401")
        } catch (failure: ClientRequestException) {
            assertEquals(HttpStatusCode.Unauthorized, failure.response.status)
        }
    }

    @Test
    fun refreshSendsTheRefreshTokenAndReturnsANewOne() = runTest {
        val engine = MockEngine { request ->
            assertEquals("/refresh", request.url.encodedPath)
            assertEquals("""{"refreshToken":"refresh-1"}""", request.body.toByteArray().decodeToString())
            respond(token.replace("refresh-1", "refresh-2").replace("access-1", "access-2"), HttpStatusCode.OK, json)
        }
        val response = AuthApi(client(engine), "https://api.example.test").refresh("refresh-1")
        assertEquals("access-2", response.accessToken)
        assertEquals("refresh-2", response.refreshToken)
    }

    @Test
    fun theClientOnlyCallsTheTwoPublicAuthRoutes() = runTest {
        val paths = mutableListOf<String>()
        val engine = MockEngine { request ->
            paths += request.url.encodedPath
            respond(token, HttpStatusCode.OK, json)
        }
        val api = AuthApi(client(engine), "https://api.example.test")
        api.login("d@x.test", "s3cret")
        api.refresh("refresh-1")
        assertEquals(listOf("/login", "/refresh"), paths)
    }
}
