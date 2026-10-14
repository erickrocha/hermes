package com.progressrail.hermes.client.tv

import android.content.Context
import android.security.KeyPairGeneratorSpec
import android.util.Base64
import java.math.BigInteger
import java.security.KeyPair
import java.security.KeyPairGenerator
import java.security.KeyStore
import java.security.PrivateKey
import java.util.Calendar
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec
import javax.security.auth.x500.X500Principal

internal data class StoredDeviceCredential(
    val accessToken: String,
    val deviceUuid: String,
    val deviceName: String,
    val expiresAt: Long,
)

internal class DeviceCredentialStore(context: Context) {
    private val appContext = context.applicationContext
    private val preferences = appContext.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
    private val keyAlias = "${appContext.packageName}.garage-device-credential"

    fun save(credential: StoredDeviceCredential) {
        val aesKey = KeyGenerator.getInstance("AES").apply { init(256) }.generateKey()
        val aesCipher = Cipher.getInstance(AES_TRANSFORMATION)
        aesCipher.init(Cipher.ENCRYPT_MODE, aesKey)
        val encryptedToken = aesCipher.doFinal(credential.accessToken.toByteArray(Charsets.UTF_8))
        val encryptedPayload = aesCipher.iv + encryptedToken

        val publicKey = getOrCreateKeyPair().public
        val wrappedKey = Cipher.getInstance(RSA_TRANSFORMATION).run {
            init(Cipher.ENCRYPT_MODE, publicKey)
            doFinal(aesKey.encoded)
        }

        preferences.edit()
            .putString(WRAPPED_KEY, encode(wrappedKey))
            .putString(ENCRYPTED_TOKEN, encode(encryptedPayload))
            .putString(DEVICE_UUID, credential.deviceUuid)
            .putString(DEVICE_NAME, credential.deviceName)
            .putLong(EXPIRES_AT, credential.expiresAt)
            .apply()
    }

    fun load(): StoredDeviceCredential? {
        val wrappedKey = preferences.getString(WRAPPED_KEY, null)?.let(::decode)
        val encryptedPayload = preferences.getString(ENCRYPTED_TOKEN, null)?.let(::decode)
        val deviceUuid = preferences.getString(DEVICE_UUID, null)
        val deviceName = preferences.getString(DEVICE_NAME, null)
        val expiresAt = preferences.getLong(EXPIRES_AT, 0L)
        if (wrappedKey == null || encryptedPayload == null || deviceUuid == null || deviceName == null) {
            return null
        }
        if (expiresAt <= System.currentTimeMillis() / 1000L) {
            clear()
            return null
        }

        return try {
            val privateKey = loadPrivateKey() ?: error("Device key is missing")
            val aesKeyBytes = Cipher.getInstance(RSA_TRANSFORMATION).run {
                init(Cipher.DECRYPT_MODE, privateKey)
                doFinal(wrappedKey)
            }
            val iv = encryptedPayload.copyOfRange(0, GCM_IV_BYTES)
            val ciphertext = encryptedPayload.copyOfRange(GCM_IV_BYTES, encryptedPayload.size)
            val accessToken = Cipher.getInstance(AES_TRANSFORMATION).run {
                init(
                    Cipher.DECRYPT_MODE,
                    SecretKeySpec(aesKeyBytes, "AES"),
                    GCMParameterSpec(GCM_TAG_BITS, iv),
                )
                doFinal(ciphertext).toString(Charsets.UTF_8)
            }
            StoredDeviceCredential(accessToken, deviceUuid, deviceName, expiresAt)
        } catch (_: Exception) {
            clear()
            null
        }
    }

    fun clear() {
        preferences.edit().clear().apply()
        val keyStore = loadKeyStore()
        if (keyStore.containsAlias(keyAlias)) {
            keyStore.deleteEntry(keyAlias)
        }
    }

    private fun getOrCreateKeyPair(): KeyPair {
        val keyStore = loadKeyStore()
        if (keyStore.containsAlias(keyAlias)) {
            val publicKey = keyStore.getCertificate(keyAlias).publicKey
            val privateKey = keyStore.getKey(keyAlias, null) as PrivateKey
            return KeyPair(publicKey, privateKey)
        }

        val start = Calendar.getInstance()
        val end = Calendar.getInstance().apply { add(Calendar.YEAR, KEY_VALIDITY_YEARS) }
        val spec = KeyPairGeneratorSpec.Builder(appContext)
            .setAlias(keyAlias)
            .setSubject(X500Principal("CN=Hermes garage device"))
            .setSerialNumber(BigInteger.ONE)
            .setStartDate(start.time)
            .setEndDate(end.time)
            .build()
        return KeyPairGenerator.getInstance("RSA", ANDROID_KEY_STORE).run {
            initialize(spec)
            generateKeyPair()
        }
    }

    private fun loadPrivateKey(): PrivateKey? {
        val keyStore = loadKeyStore()
        return if (keyStore.containsAlias(keyAlias)) {
            keyStore.getKey(keyAlias, null) as? PrivateKey
        } else {
            null
        }
    }

    private fun loadKeyStore(): KeyStore = KeyStore.getInstance(ANDROID_KEY_STORE).apply {
        load(null)
    }

    private fun encode(bytes: ByteArray): String = Base64.encodeToString(bytes, Base64.NO_WRAP)

    private fun decode(value: String): ByteArray? = try {
        Base64.decode(value, Base64.NO_WRAP)
    } catch (_: IllegalArgumentException) {
        null
    }

    private companion object {
        const val ANDROID_KEY_STORE = "AndroidKeyStore"
        const val AES_TRANSFORMATION = "AES/GCM/NoPadding"
        const val RSA_TRANSFORMATION = "RSA/ECB/PKCS1Padding"
        const val GCM_IV_BYTES = 12
        const val GCM_TAG_BITS = 128
        const val KEY_VALIDITY_YEARS = 30
        const val PREFERENCES = "garage_device_credential"
        const val WRAPPED_KEY = "wrapped_key"
        const val ENCRYPTED_TOKEN = "encrypted_token"
        const val DEVICE_UUID = "device_uuid"
        const val DEVICE_NAME = "device_name"
        const val EXPIRES_AT = "expires_at"
    }
}