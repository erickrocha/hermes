package com.erocha.hermes.client.driver

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.util.Log
import com.erocha.hermes.client.shared.FuelReceiptParser
import kotlinx.coroutines.runBlocking
import java.io.File

/**
 * Debug builds only: `adb shell am broadcast -n .../.DebugScanReceiver --es path /data/local/tmp/receipt.jpg`
 * runs the same recognizer and parser the camera flow uses on a photo pushed with adb, and logs the outcome.
 */
class DebugScanReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val path = intent.getStringExtra("path") ?: return
        val copy = File.createTempFile("debug-scan-", ".jpg", context.cacheDir).also { File(path).copyTo(it, overwrite = true) }
        val pending = goAsync()
        Thread {
            try {
                val text = runBlocking { ReceiptScanner.read(context, copy, Uri.fromFile(copy)) }
                Log.i("DebugScan", "TEXT<<\n$text\n>>")
                Log.i("DebugScan", "PARSED ${FuelReceiptParser.parse(text)}")
            } catch (failure: Exception) {
                Log.e("DebugScan", "failed", failure)
            } finally {
                pending.finish()
            }
        }.start()
    }
}
