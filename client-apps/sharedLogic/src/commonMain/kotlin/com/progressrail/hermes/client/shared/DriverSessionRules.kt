package com.progressrail.hermes.client.shared

const val DRIVER_ROLE = "Driver"
const val DEFAULT_REFRESH_MARGIN_SECONDS = 300L

/** Interface courtesy only; the server decides authorization (HRMS-505). */
fun isDriverRole(role: String): Boolean = role == DRIVER_ROLE

/** [expireIn] is an absolute Unix instant in seconds; renew once it is within [marginSeconds]. */
fun shouldRefresh(expireIn: Long, nowSeconds: Long, marginSeconds: Long = DEFAULT_REFRESH_MARGIN_SECONDS): Boolean =
    nowSeconds >= expireIn - marginSeconds

/** Seconds to wait before the next renewal; zero when it is already due. */
fun secondsUntilRefresh(expireIn: Long, nowSeconds: Long, marginSeconds: Long = DEFAULT_REFRESH_MARGIN_SECONDS): Long =
    maxOf(0L, expireIn - marginSeconds - nowSeconds)
