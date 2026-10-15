package com.progressrail.hermes.client.tv

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/** Reopens the wall monitor after a TV reboot; the display runs unattended. */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action != Intent.ACTION_BOOT_COMPLETED) return
        context.startActivity(
            Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK),
        )
    }
}
