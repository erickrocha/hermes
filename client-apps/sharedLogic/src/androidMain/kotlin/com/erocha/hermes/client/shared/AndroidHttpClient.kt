package com.erocha.hermes.client.shared

import io.ktor.client.HttpClient
import io.ktor.client.engine.android.Android
import io.ktor.client.plugins.HttpTimeout
import io.ktor.client.plugins.contentnegotiation.ContentNegotiation
import io.ktor.client.plugins.defaultRequest
import io.ktor.client.request.header
import io.ktor.http.HttpHeaders
import io.ktor.serialization.kotlinx.json.json
import kotlinx.serialization.json.Json

/** The shared client for every Hermes app; the garage TV keeps its historical name below. */
/** [acceptLanguage], when given, makes the server word its error messages in that language (`SR-CA-03-06`). */
fun createHermesHttpClient(acceptLanguage: String? = null): HttpClient = HttpClient(Android) {
    // The activity branches on ClientRequestException (401 -> re-pair); without this a 4xx is parsed as the body.
    expectSuccess = true
    install(HttpTimeout) {
        requestTimeoutMillis = 20_000
        connectTimeoutMillis = 10_000
        socketTimeoutMillis = 20_000
    }
    acceptLanguage?.let { language -> defaultRequest { header(HttpHeaders.AcceptLanguage, language) } }
    install(ContentNegotiation) {
        json(Json {
            ignoreUnknownKeys = true
        })
    }
}

fun createGarageDeviceHttpClient(): HttpClient = createHermesHttpClient()
