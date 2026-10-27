package com.erocha.hermes.client.driver

import android.content.Context
import android.net.Uri
import androidx.core.content.FileProvider
import com.google.mlkit.vision.common.InputImage
import com.google.mlkit.vision.text.TextRecognition
import com.google.mlkit.vision.text.latin.TextRecognizerOptions
import kotlinx.coroutines.suspendCancellableCoroutine
import java.io.File
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/**
 * The photo of a fuel receipt, read on the phone and thrown away: the picture goes into the cache for the camera
 * app to write, is recognized on-device, and is deleted straight after. Nothing is stored or sent.
 */
internal object ReceiptScanner {
    fun newPhoto(context: Context): Pair<File, Uri> {
        val dir = File(context.cacheDir, "receipts").apply { mkdirs() }
        val file = File.createTempFile("receipt-", ".jpg", dir)
        return file to FileProvider.getUriForFile(context, "${context.packageName}.files", file)
    }

    /** The text the recognizer found; [photo] is deleted whether or not it succeeded. */
    suspend fun read(context: Context, photo: File, uri: Uri): String = try {
        recognize(context, uri)
    } finally {
        photo.delete()
    }

    private suspend fun recognize(context: Context, uri: Uri): String = suspendCancellableCoroutine { continuation ->
        val recognizer = TextRecognition.getClient(TextRecognizerOptions.DEFAULT_OPTIONS)
        val image = try {
            InputImage.fromFilePath(context, uri)
        } catch (failure: Exception) {
            recognizer.close()
            continuation.resumeWithException(failure)
            return@suspendCancellableCoroutine
        }
        recognizer.process(image)
            .addOnSuccessListener { text -> recognizer.close(); continuation.resume(text.text) }
            .addOnFailureListener { failure -> recognizer.close(); continuation.resumeWithException(failure) }
        continuation.invokeOnCancellation { recognizer.close() }
    }
}
