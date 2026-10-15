package com.progressrail.hermes.client.driver

import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

/** Server instants are UTC ISO-8601 (with or without `Z`); the screens show the device's local time (`SR-CA-03-12`). */
internal object Instants {
    private const val WIRE = "yyyy-MM-dd'T'HH:mm:ss"

    /** The current instant as the server's `NaiveDateTime` (UTC, no zone). */
    fun nowUtc(now: Date = Date()): String = SimpleDateFormat(WIRE, Locale.US).apply { timeZone = TimeZone.getTimeZone("UTC") }.format(now)

    /** "2026-10-15T12:00:00Z" -> "15/10/2026 09:00" in [zone]; the input is returned unchanged when it cannot be read. */
    fun toLocal(iso: String?, zone: TimeZone = TimeZone.getDefault()): String {
        if (iso.isNullOrBlank()) return ""
        val seconds = iso.trimEnd('Z').substringBefore('.').take(WIRE.length - 2)
        val parsed = runCatching { SimpleDateFormat(WIRE, Locale.US).apply { timeZone = TimeZone.getTimeZone("UTC") }.parse(seconds) }.getOrNull()
            ?: return iso
        return SimpleDateFormat("dd/MM/yyyy HH:mm", Locale("pt", "BR")).apply { timeZone = zone }.format(parsed)
    }

    /** A business date `yyyy-MM-dd` has no zone: "2026-11-20" -> "20/11/2026". */
    fun date(value: String?): String = value?.split('-')?.takeIf { it.size == 3 }?.let { "${it[2]}/${it[1]}/${it[0]}" } ?: value.orEmpty()

    /** "08:00:00" -> "08:00". */
    fun time(value: String?): String = value?.take(5).orEmpty()

    /** 12345 centavos -> "123,45". */
    fun money(cents: Long): String = String.format(Locale("pt", "BR"), "%,.2f", cents / 100.0)
}
