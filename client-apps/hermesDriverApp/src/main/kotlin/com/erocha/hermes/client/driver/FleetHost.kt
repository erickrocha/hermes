package com.erocha.hermes.client.driver

import android.content.Context
import com.erocha.hermes.client.shared.AppAccess
import com.erocha.hermes.client.shared.FleetApi
import com.erocha.hermes.client.shared.GarageApi
import com.erocha.hermes.client.shared.WorkshopApi

/** The server refused the call; [message] is its localized reason (`SR-CA-03-07`), when it gave one. */
internal class FleetRefused(val status: Int, val reason: String?, val errorKey: String = "") : Exception(reason)

/** No usable answer arrived (network, timeout): the outcome on the server is unknown. */
internal class FleetUnreachable(cause: Throwable) : Exception(cause)

/** The session is over (renewal refused); the host has already returned to the login. */
internal class SessionEnded(val messageRes: Int?) : Exception()

/** What the fleet screens need from the activity. */
internal interface FleetHost {
    val context: Context
    val userUuid: String

    /** The session's grants, deciding which screens and buttons are offered (`C-035`). */
    val access: AppAccess

    /**
     * Runs [work] off the main thread with a valid access token (one renewal and one repetition on `401`,
     * `SR-CA-03-04`) and delivers the outcome on the main thread. A session that ends is handled by the host and
     * never delivered.
     */
    fun <T> run(work: suspend (api: FleetApi, accessToken: String) -> T, onDone: (Result<T>) -> Unit)

    /** Same as [run], for the garage-yard routes the mechanic uses. */
    fun <T> runGarage(work: suspend (api: GarageApi, accessToken: String) -> T, onDone: (Result<T>) -> Unit)

    /** Same as [run], for the valeta and work-order routes the mechanic uses. */
    fun <T> runWorkshop(work: suspend (api: WorkshopApi, accessToken: String) -> T, onDone: (Result<T>) -> Unit)

    /** Opens [latitude]/[longitude] in the device's map app; false when there is none. */
    fun openMap(latitude: Double, longitude: Double): Boolean
}
