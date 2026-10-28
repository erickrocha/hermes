package com.erocha.hermes.client.driver

import android.content.Context
import com.erocha.hermes.client.shared.DriverSession
import com.erocha.hermes.client.shared.EncryptedSecretStore
import com.erocha.hermes.client.shared.decodeDriverSession
import com.erocha.hermes.client.shared.encode

/** The driver's session (access and refresh token) kept encrypted at rest (SR-CA-01-01). */
internal class DriverSessionStore(context: Context) {
    private val secrets = EncryptedSecretStore(
        context = context,
        preferencesName = "driver_session",
        keyAlias = "${context.applicationContext.packageName}.driver-session",
        keySubject = "CN=Hermes driver session",
    )

    fun save(session: DriverSession) = secrets.save(session.encode())

    /** Null when absent or unreadable; an unreadable session (including one saved before `C-035`, without grants) is discarded. */
    fun load(): DriverSession? {
        val raw = secrets.load() ?: return null
        return decodeDriverSession(raw) ?: run {
            clear()
            null
        }
    }

    fun clear() = secrets.clear()
}
