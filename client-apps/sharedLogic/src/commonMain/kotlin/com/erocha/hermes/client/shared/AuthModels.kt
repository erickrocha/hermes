package com.erocha.hermes.client.shared

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

const val SCOPE_PLATFORM = "Platform"
const val SCOPE_TENANT = "Tenant"

/**
 * `POST /login` and `POST /refresh` response (`C-035`): the former `role` gave way to the access profile and its
 * effective [permissions]. Only the token fields, the name, the uuid and the grants are used by the driver shell.
 */
@Serializable
data class AccessTokenResponse(
    val accessToken: String,
    /** Absolute Unix instant in seconds (UTC), not a duration. */
    val expireIn: Long,
    val refreshToken: String? = null,
    val name: String = "",
    /** `Platform` or `Tenant`. */
    val scope: String = SCOPE_TENANT,
    /** System profile key (`Driver`, `Mechanic`, ...) or absent for a tenant's own profile. */
    val profileKey: String? = null,
    val profileName: String? = null,
    /** Effective grants: a permission `code` or `code@own` (own records only). */
    val permissions: List<String> = emptyList(),
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
    /**
     * Effective grants (`code` or `code@own`), the basis of every gate in this app. Required on purpose: a session
     * saved before `C-035` carried a `role` and no grants, so it fails to decode and the person signs in again.
     */
    val permissions: List<String>,
    /** The user's own uuid (`SR-CA-03-02`). Empty in a session saved before `C-031`; a refresh fills it. */
    val uuid: String = "",
    val profileName: String? = null,
)

private val sessionJson = Json { ignoreUnknownKeys = true }

fun DriverSession.encode(): String = sessionJson.encodeToString(DriverSession.serializer(), this)

fun decodeDriverSession(value: String): DriverSession? =
    runCatching { sessionJson.decodeFromString(DriverSession.serializer(), value) }.getOrNull()
