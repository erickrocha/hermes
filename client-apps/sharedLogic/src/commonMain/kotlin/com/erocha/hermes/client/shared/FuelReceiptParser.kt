package com.erocha.hermes.client.shared

import kotlin.math.abs
import kotlin.math.roundToLong

/**
 * What could be read from a fuel-station receipt (NFC-e / cupom fiscal). Every field is optional: the driver sees
 * what was understood, fixes what was not, and the server still validates the fuelling (`TRM-520/542`).
 */
data class ScannedReceipt(
    val volumeLiters: Double? = null,
    val valueCents: Long? = null,
    val station: String? = null,
    /** Local time printed on the receipt, `yyyy-MM-ddTHH:mm:ss`; null when it was not read. */
    val dateTime: String? = null,
    val fuel: String? = null,
    val cnpj: String? = null,
) {
    val isEmpty get() = volumeLiters == null && valueCents == null && station == null && dateTime == null
}

/**
 * Reads the text a recognizer found on a Brazilian fuel receipt. It is deliberately forgiving about layout (the
 * item line can be split in two by the recognizer, `R$` can read `RS`) and strict about sense: a quantity is only
 * trusted next to a price and a total that agree with each other.
 */
object FuelReceiptParser {
    // The recognizer sometimes splits "266,81" into "266, 81".
    private val amount = Regex("""(?:\d{1,3}(?:\.\d{3})+|\d+),\s?\d{2}""")
    private val decimal = Regex("""\d+(?:\.\d{3})*,\d+|\d+\.\d+""")
    private val fuelWord = Regex("""DIESEL|GASOLINA|ETANOL|ALCOOL|ÁLCOOL|\bGNV\b|\bS-?10\b|\bS-?500\b""")
    private val totalWord = Regex("""VALOR\s+TOTAL|VL\.?\s*TOTAL|TOTAL\s+R.|TOTAL\s+A\s+PAGAR|VALOR\s+A\s+PAGAR|VALOR\s+PAGO|TOTAL\s*:""")
    private val litersUnit = Regex("""(\d+(?:[.,]\d{1,3}))\s*(?:LTS?|LTR|LITROS?|L)\b""")
    private val cnpjRegex = Regex("""\d{2}\.\d{3}\.\d{3}/\d{4}-\d{2}""")
    private val dateTime = Regex("""(\d{2})/(\d{2})/(\d{4})[\sT,-]*(\d{2}):(\d{2})(?::(\d{2}))?""")
    private val notAName = Regex("""CNPJ|CPF|CUPOM|NOTA|DANFE|DOCUMENTO|NFC|^\d|ENDERE|\bRUA\b|\bAV\b|AVENIDA|\bCEP\b|FONE|\bTEL\b|\bIE\b|INSC""")

    fun parse(text: String): ScannedReceipt {
        val lines = text.lines().map { it.trim() }.filter { it.isNotEmpty() }
        val upper = lines.map { it.uppercase() }
        val item = itemLine(lines, upper)
        val total = declaredTotal(lines, upper) ?: item?.total?.let { (it * 100).roundToLong() }
            ?: lines.flatMap { amounts(it) }.maxOrNull()?.let { (it * 100).roundToLong() }
        return ScannedReceipt(
            volumeLiters = item?.liters ?: litersByUnit(upper),
            valueCents = total,
            station = station(lines, upper),
            dateTime = dateTime(lines),
            fuel = item?.fuel,
            cnpj = cnpjRegex.find(text)?.value,
        )
    }

    private data class Item(val fuel: String, val liters: Double, val total: Double)

    /** The fuel item: a quantity, a unit price and a total where quantity × price matches the total. */
    private fun itemLine(lines: List<String>, upper: List<String>): Item? {
        for (index in upper.indices) {
            val fuel = fuelWord.find(upper[index])?.value ?: continue
            if (upper[index].contains("ARLA")) continue
            val window = (index..minOf(index + 2, lines.lastIndex)).joinToString(" ") { lines[it] }
            val numbers = decimal.findAll(window).mapNotNull { number(it.value) }.toList()
            for (at in 0..numbers.size - 3) {
                val (quantity, price, total) = numbers.subList(at, at + 3)
                if (quantity > 0 && price > 0 && abs(quantity * price - total) <= maxOf(0.06, total * 0.002)) {
                    return Item(fuel, quantity, total)
                }
            }
        }
        return null
    }

    private fun declaredTotal(lines: List<String>, upper: List<String>): Long? {
        for (index in upper.indices) {
            if (!totalWord.containsMatchIn(upper[index])) continue
            // The amount is on the same line, or on the next one when the recognizer split the row.
            val found = amounts(lines[index]).lastOrNull() ?: lines.getOrNull(index + 1)?.let(::amounts)?.lastOrNull()
            if (found != null) return (found * 100).roundToLong()
        }
        return null
    }

    private fun litersByUnit(upper: List<String>): Double? =
        upper.firstNotNullOfOrNull { line -> litersUnit.find(line)?.groupValues?.get(1)?.let(::number)?.takeIf { it > 0 } }

    private fun station(lines: List<String>, upper: List<String>): String? =
        lines.indices.take(6).firstOrNull { index ->
            upper[index].count { it.isLetter() } >= 4 && !notAName.containsMatchIn(upper[index])
        }?.let { lines[it].take(80) }

    private fun dateTime(lines: List<String>): String? {
        val text = lines.joinToString("\n")
        // The issue instant is the one printed next to "Emissão"; any other date on the receipt comes second.
        val emission = Regex("""EMISS[ÃA]O[^\n]{0,20}""", RegexOption.IGNORE_CASE).find(text)?.let { dateTime.find(text, it.range.first) }
        val match = emission ?: dateTime.find(text) ?: return null
        val (day, month, year, hour, minute, second) = match.destructured
        if (month.toInt() !in 1..12 || day.toInt() !in 1..31 || hour.toInt() > 23 || minute.toInt() > 59) return null
        return "$year-$month-${day}T$hour:$minute:${second.ifEmpty { "00" }}"
    }

    private fun amounts(line: String): List<Double> = amount.findAll(line).mapNotNull { number(it.value) }.toList()

    /** "1.234,56" -> 1234.56; "45,230" -> 45.23; "5.899" is a decimal point, not a thousands separator. */
    private fun number(raw: String): Double? {
        val normalized = if (raw.contains(',')) raw.replace(" ", "").replace(".", "").replace(',', '.') else raw
        return normalized.toDoubleOrNull()
    }
}
