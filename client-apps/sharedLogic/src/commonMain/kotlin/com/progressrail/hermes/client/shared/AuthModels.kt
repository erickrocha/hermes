package com.progressrail.hermes.client.shared

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

/** `POST /login` and `POST /refresh` response; only the first five fields are used by the driver shell. */
@Serializable
data class AccessTokenResponse(
    val accessToken: String,
    /** Absolute Unix instant in seconds (UTC), not a duration. */
    val expireIn: Long,
    val refreshToken: String? = null,
    val name: String = "",
    val role: String,
    val tokenType: String = "Bearer",
    val email: String = "",
    val uuid: String = "",
    val userId: Long = 0,
    val tenantId: Long? = null,
    val tenantUuid: String? = null,
)

@Serializable
data class RefreshTokenRequest(val refreshToken: String)

/** What the driver app keeps between launches (stored encrypted). */
@Serializable
data class DriverSession(
    val accessToken: String,
    val refreshToken: String,
    val expireIn: Long,
    val name: String,
    val role: String,
)

private val sessionJson = Json { ignoreUnknownKeys = true }

fun DriverSession.encode(): String = sessionJson.encodeToString(DriverSession.serializer(), this)

fun decodeDriverSession(value: String): DriverSession? =
    runCatching { sessionJson.decodeFromString(DriverSession.serializer(), value) }.getOrNull()
