package com.progressrail.hermes.client.shared

import android.content.Context
import android.content.SharedPreferences
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

/**
 * One secret encrypted at rest: AES-GCM for the value, the AES key wrapped by an RSA key kept in the Android
 * Keystore. Extracted from the garage TV shell; the on-disk layout (`wrapped_key`, `encrypted_token`) is unchanged
 * so an already-paired TV keeps working after an update.
 */
class EncryptedSecretStore(
    context: Context,
    preferencesName: String,
    private val keyAlias: String,
    private val keySubject: String,
) {
    private val appContext = context.applicationContext
    val preferences: SharedPreferences = appContext.getSharedPreferences(preferencesName, Context.MODE_PRIVATE)

    /** Stores [secret] and, in the same commit, whatever non-secret [extras] the caller keeps beside it. */
    fun save(secret: String, extras: SharedPreferences.Editor.() -> Unit = {}) {
        val aesKey = KeyGenerator.getInstance("AES").apply { init(256) }.generateKey()
        val aesCipher = Cipher.getInstance(AES_TRANSFORMATION)
        aesCipher.init(Cipher.ENCRYPT_MODE, aesKey)
        val encryptedSecret = aesCipher.doFinal(secret.toByteArray(Charsets.UTF_8))
        val encryptedPayload = aesCipher.iv + encryptedSecret

        val wrappedKey = Cipher.getInstance(RSA_TRANSFORMATION).run {
            init(Cipher.ENCRYPT_MODE, getOrCreateKeyPair().public)
            doFinal(aesKey.encoded)
        }

        preferences.edit()
            .putString(WRAPPED_KEY, encode(wrappedKey))
            .putString(ENCRYPTED_TOKEN, encode(encryptedPayload))
            .apply(extras)
            .apply()
    }

    /** The decrypted secret, or null if absent. Anything that cannot be decrypted is discarded. */
    fun load(): String? {
        val wrappedKey = preferences.getString(WRAPPED_KEY, null)?.let(::decode)
        val encryptedPayload = preferences.getString(ENCRYPTED_TOKEN, null)?.let(::decode)
        if (wrappedKey == null || encryptedPayload == null) return null
        return try {
            val privateKey = loadPrivateKey() ?: error("Key is missing")
            val aesKeyBytes = Cipher.getInstance(RSA_TRANSFORMATION).run {
                init(Cipher.DECRYPT_MODE, privateKey)
                doFinal(wrappedKey)
            }
            val iv = encryptedPayload.copyOfRange(0, GCM_IV_BYTES)
            val ciphertext = encryptedPayload.copyOfRange(GCM_IV_BYTES, encryptedPayload.size)
            Cipher.getInstance(AES_TRANSFORMATION).run {
                init(Cipher.DECRYPT_MODE, SecretKeySpec(aesKeyBytes, "AES"), GCMParameterSpec(GCM_TAG_BITS, iv))
                doFinal(ciphertext).toString(Charsets.UTF_8)
            }
        } catch (_: Exception) {
            clear()
            null
        }
    }

    fun clear() {
        preferences.edit().clear().apply()
        val keyStore = loadKeyStore()
        if (keyStore.containsAlias(keyAlias)) keyStore.deleteEntry(keyAlias)
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
        // shortcut: KeyPairGeneratorSpec is deprecated but is what minSdk 21 offers; revisit when minSdk reaches 23.
        @Suppress("DEPRECATION")
        val spec = KeyPairGeneratorSpec.Builder(appContext)
            .setAlias(keyAlias)
            .setSubject(X500Principal(keySubject))
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
        return if (keyStore.containsAlias(keyAlias)) keyStore.getKey(keyAlias, null) as? PrivateKey else null
    }

    private fun loadKeyStore(): KeyStore = KeyStore.getInstance(ANDROID_KEY_STORE).apply { load(null) }

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
        const val WRAPPED_KEY = "wrapped_key"
        const val ENCRYPTED_TOKEN = "encrypted_token"
    }
}
