package com.erocha.hermes.client.shared

import io.ktor.client.HttpClient
import io.ktor.client.call.body
import io.ktor.client.request.forms.submitForm
import io.ktor.client.request.post
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.contentType
import io.ktor.http.parameters

/** The platform's existing public authentication routes; the app has no identity of its own (HRMS-503). */
class AuthApi(
    private val httpClient: HttpClient,
    private val baseUrl: String,
) {
    suspend fun login(email: String, password: String): AccessTokenResponse =
        httpClient.submitForm(
            url = "${baseUrl.trimEnd('/')}/login",
            formParameters = parameters {
                append("email", email)
                append("password", password)
            },
        ).body()

    suspend fun refresh(refreshToken: String): AccessTokenResponse =
        httpClient.post("${baseUrl.trimEnd('/')}/refresh") {
            contentType(ContentType.Application.Json)
            setBody(RefreshTokenRequest(refreshToken))
        }.body()
}
