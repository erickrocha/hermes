package com.progressrail.hermes.client.tv

import android.content.Context
import com.progressrail.hermes.client.shared.EncryptedSecretStore

internal data class StoredDeviceCredential(
    val accessToken: String,
    val deviceUuid: String,
    val deviceName: String,
    val expiresAt: Long,
)

/** The token is encrypted by the shared store; the device metadata stays in the same preferences file. */
internal class DeviceCredentialStore(context: Context) {
    private val secrets = EncryptedSecretStore(
        context = context,
        preferencesName = PREFERENCES,
        keyAlias = "${context.applicationContext.packageName}.garage-device-credential",
        keySubject = "CN=Hermes garage device",
    )
    private val preferences = secrets.preferences

    fun save(credential: StoredDeviceCredential) {
        secrets.save(credential.accessToken) {
            putString(DEVICE_UUID, credential.deviceUuid)
            putString(DEVICE_NAME, credential.deviceName)
            putLong(EXPIRES_AT, credential.expiresAt)
        }
    }

    fun load(): StoredDeviceCredential? {
        val deviceUuid = preferences.getString(DEVICE_UUID, null)
        val deviceName = preferences.getString(DEVICE_NAME, null)
        val expiresAt = preferences.getLong(EXPIRES_AT, 0L)
        if (deviceUuid == null || deviceName == null) return null
        if (expiresAt <= System.currentTimeMillis() / 1000L) {
            clear()
            return null
        }
        val accessToken = secrets.load() ?: return null
        return StoredDeviceCredential(accessToken, deviceUuid, deviceName, expiresAt)
    }

    fun clear() = secrets.clear()

    private companion object {
        const val PREFERENCES = "garage_device_credential"
        const val DEVICE_UUID = "device_uuid"
        const val DEVICE_NAME = "device_name"
        const val EXPIRES_AT = "expires_at"
    }
}
