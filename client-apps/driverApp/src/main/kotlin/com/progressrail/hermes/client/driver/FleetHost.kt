package com.progressrail.hermes.client.driver

import android.content.Context
import android.widget.LinearLayout
import android.widget.TextView
import com.progressrail.hermes.client.shared.FleetApi

/** The server refused the call; [message] is its localized reason (`SR-CA-03-07`), when it gave one. */
internal class FleetRefused(val status: Int, val reason: String?) : Exception(reason)

/** No usable answer arrived (network, timeout): the outcome on the server is unknown. */
internal class FleetUnreachable(cause: Throwable) : Exception(cause)

/** The session is over (renewal refused); the host has already returned to the login. */
internal class SessionEnded(val messageRes: Int?) : Exception()

/** What the fleet screens need from the activity. */
internal interface FleetHost {
    val context: Context
    val userUuid: String

    /**
     * Runs [work] off the main thread with a valid access token (one renewal and one repetition on `401`,
     * `SR-CA-03-04`) and delivers the outcome on the main thread. A session that ends is handled by the host and
     * never delivered.
     */
    fun <T> run(work: suspend (api: FleetApi, accessToken: String) -> T, onDone: (Result<T>) -> Unit)

    fun text(value: String, appearance: Int): TextView
    fun spacing(): LinearLayout.LayoutParams
}
