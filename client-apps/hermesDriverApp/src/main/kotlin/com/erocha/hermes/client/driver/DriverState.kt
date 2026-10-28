package com.erocha.hermes.client.driver

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateMapOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.erocha.hermes.client.shared.ANSWER_CONFORMING
import com.erocha.hermes.client.shared.CHECKLIST_DEPARTURE
import com.erocha.hermes.client.shared.CHECKLIST_RETURN
import com.erocha.hermes.client.shared.ChecklistTemplate
import com.erocha.hermes.client.shared.ExtraTrip
import com.erocha.hermes.client.shared.FuelEntry
import com.erocha.hermes.client.shared.ScannedReceipt
import com.erocha.hermes.client.shared.Vehicle
import com.erocha.hermes.client.shared.VehicleAction
import com.erocha.hermes.client.shared.VehicleTracking
import com.erocha.hermes.client.shared.buildChecklistRun
import com.erocha.hermes.client.shared.buildFuelReceipt
import com.erocha.hermes.client.shared.canSubmitChecklist
import com.erocha.hermes.client.shared.canSubmitReceipt
import com.erocha.hermes.client.shared.defaultTemplate
import com.erocha.hermes.client.shared.parseOdometer
import com.erocha.hermes.client.shared.vehicleAction
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

internal data class VehicleRow(val vehicle: Vehicle, val action: VehicleAction)

/** The checklist being filled: answers live in snapshot maps so the cards redraw as the driver taps. */
internal class ChecklistForm(val row: VehicleRow, val template: ChecklistTemplate) {
    val answers = mutableStateMapOf<String, String>()
    val observations = mutableStateMapOf<String, String>()
    var odometer by mutableStateOf("")
    var notes by mutableStateOf("")
    var submitting by mutableStateOf(false)
    /** Set after a send with no answer: the server may or may not have recorded it (`SR-CA-03-03`). */
    var needsCheck by mutableStateOf(false)
    var message by mutableStateOf<String?>(null)

    val canSubmit get() = !submitting && canSubmitChecklist(odometer, template.items, answers)
    val isReturn get() = template.checklistType == CHECKLIST_RETURN
}

internal class ReceiptForm(var vehicle: Vehicle) {
    var prefix by mutableStateOf(vehicle.prefix.orEmpty())
    var liters by mutableStateOf("")
    var value by mutableStateOf("")
    var odometer by mutableStateOf("")
    var station by mutableStateOf("")
    var fullTank by mutableStateOf(false)
    /** The receipt's own time (local, `yyyy-MM-ddTHH:mm:ss`) when it was read; otherwise the fuelling is stamped now. */
    var printedAt by mutableStateOf<String?>(null)
    var submitting by mutableStateOf(false)
    var scanning by mutableStateOf(false)
    /** What the scan understood, so the driver knows to check the fields; null before any scan. */
    var scanned by mutableStateOf<ScannedReceipt?>(null)
    var scanFailed by mutableStateOf(false)
    /** Set after `OdometerOutsideWindow`: the driver may confirm the reading and send anyway. */
    var canOverride by mutableStateOf(false)
    var message by mutableStateOf<String?>(null)

    val canSubmit get() = !submitting && !scanning && canSubmitReceipt(prefix, liters, value, odometer)

    fun choose(chosen: Vehicle) {
        vehicle = chosen
        prefix = chosen.prefix.orEmpty()
    }

    fun apply(result: ScannedReceipt) {
        scanned = result
        scanFailed = result.volumeLiters == null && result.valueCents == null
        result.volumeLiters?.let { liters = formatLiters(it) }
        result.valueCents?.let { value = formatMoneyField(it) }
        result.station?.let { station = it }
        printedAt = result.dateTime
    }
}

/** 45.23 -> "45,23": the way the driver types it, without the zeros of a 3-decimal reading. */
internal fun formatLiters(value: Double): String =
    String.format(Locale.US, "%.3f", value).trimEnd('0').trimEnd('.').replace('.', ',')

/** 26681 centavos -> "266,81". */
internal fun formatMoneyField(cents: Long): String = String.format(Locale.US, "%.2f", cents / 100.0).replace('.', ',')

internal sealed interface DriverScreen {
    data object Tabs : DriverScreen
    class Checklist(val form: ChecklistForm) : DriverScreen
    class Position(val vehicle: Vehicle) : DriverScreen
    class Receipt(val form: ReceiptForm) : DriverScreen
}

/** The driver's screen state; every call goes through the host, which renews the session and reports refusals. */
internal class DriverState(private val host: FleetHost) {
    val access get() = host.access
    var screen by mutableStateOf<DriverScreen>(DriverScreen.Tabs)
    var rows by mutableStateOf<List<VehicleRow>?>(null)
    var homeMessage by mutableStateOf<String?>(null)
    var refreshing by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)
    var busy by mutableStateOf(false)

    var trips by mutableStateOf<List<ExtraTrip>?>(null)
    var entries by mutableStateOf<List<FuelEntry>?>(null)

    var tracking by mutableStateOf<VehicleTracking?>(null)
    var trackingError by mutableStateOf<String?>(null)

    private fun message(failure: Throwable, fallback: Int) = failureMessage(host, failure, fallback)
    private fun string(res: Int) = host.context.getString(res)

    fun loadHome(notice: String? = null) {
        if (notice != null) homeMessage = notice
        refreshing = true
        host.run({ api, token ->
            api.vehicles(token).map { VehicleRow(it, vehicleAction(api.possession(token, it.uuid), host.userUuid)) }
        }) { result ->
            refreshing = false
            result.onSuccess { rows = it; error = null }.onFailure { error = message(it, R.string.load_failed) }
        }
    }

    fun loadTrips() {
        refreshing = true
        host.run({ api, token -> api.trips(token) }) { result ->
            refreshing = false
            result.onSuccess { trips = it; error = null }.onFailure { error = message(it, R.string.load_failed) }
        }
    }

    fun loadEntries() {
        refreshing = true
        host.run({ api, token -> api.fuelEntries(token) }) { result ->
            refreshing = false
            result.onSuccess { entries = it; error = null }.onFailure { error = message(it, R.string.load_failed) }
        }
    }

    fun backToTabs(notice: String? = null) {
        screen = DriverScreen.Tabs
        homeMessage = notice
        loadHome()
        loadEntries()
    }

    // ---- checklist ----

    fun openChecklist(row: VehicleRow) {
        val type = if (row.action is VehicleAction.Return) CHECKLIST_RETURN else CHECKLIST_DEPARTURE
        busy = true
        error = null
        host.run({ api, token -> defaultTemplate(api.templates(token), type)?.let { api.template(token, it.uuid) } }) { result ->
            busy = false
            result.onSuccess { template ->
                if (template == null) error = string(R.string.no_default_template) else screen = DriverScreen.Checklist(ChecklistForm(row, template))
            }.onFailure { error = message(it, R.string.load_failed) }
        }
    }

    fun submit(form: ChecklistForm) {
        val km = parseOdometer(form.odometer) ?: return
        val run = buildChecklistRun(
            form.template, host.userUuid, form.row.vehicle.uuid, form.row.action, km,
            form.answers.toMap(), form.observations.toMap(), form.notes,
        )
        form.submitting = true
        form.message = null
        host.run({ api, token -> api.submitRun(token, run) }) { result ->
            form.submitting = false
            result.onSuccess { backToTabs(string(R.string.submitted)) }.onFailure { failure ->
                if (failure is FleetUnreachable) {
                    form.needsCheck = true
                    form.message = string(R.string.submit_unreachable)
                } else {
                    form.message = message(failure, R.string.submit_refused)
                }
            }
        }
    }

    /** After an unanswered send: ask the server whether it was recorded before allowing another. */
    fun check(form: ChecklistForm) {
        form.submitting = true
        host.run({ api, token -> vehicleAction(api.possession(token, form.row.vehicle.uuid), host.userUuid) }) { result ->
            form.submitting = false
            result.onSuccess { now ->
                if (now != form.row.action) {
                    backToTabs(string(R.string.already_registered))
                } else {
                    form.needsCheck = false
                    form.message = string(R.string.not_yet_registered)
                }
            }.onFailure { form.message = string(R.string.submit_unreachable) }
        }
    }

    // ---- position ----

    fun openPosition(vehicle: Vehicle) {
        screen = DriverScreen.Position(vehicle)
        refreshPosition(vehicle)
    }

    fun refreshPosition(vehicle: Vehicle) {
        refreshing = true
        trackingError = null
        host.run({ api, token -> api.tracking(token, vehicle.uuid) }) { result ->
            refreshing = false
            result.onSuccess { tracking = it }.onFailure { failure ->
                tracking = null
                trackingError = when {
                    failure is FleetRefused && failure.errorKey == "TrackerNotLinked" -> string(R.string.tracker_not_linked)
                    failure is FleetRefused && failure.status == 503 -> string(R.string.tracking_unavailable)
                    else -> message(failure, R.string.load_failed)
                }
            }
        }
    }

    // ---- fuel receipt ----

    fun openReceipt(vehicle: Vehicle? = null) {
        val chosen = vehicle ?: rows?.firstOrNull()?.vehicle ?: return
        screen = DriverScreen.Receipt(ReceiptForm(chosen))
    }

    fun submitReceipt(form: ReceiptForm, override: Boolean) {
        val request = buildFuelReceipt(
            form.vehicle.uuid, form.prefix, recordedAtUtc(form.printedAt), form.liters, form.value, form.odometer, form.station, form.fullTank, override,
        ) ?: return
        form.submitting = true
        form.message = null
        host.run({ api, token -> api.submitReceipt(token, request) }) { result ->
            form.submitting = false
            result.onSuccess {
                backToTabs(string(if (it.alreadyRecorded) R.string.fuel_already_registered else R.string.fuel_registered))
            }.onFailure { failure ->
                form.canOverride = failure is FleetRefused && failure.errorKey == "OdometerOutsideWindow"
                form.message = if (failure is FleetUnreachable) string(R.string.fuel_unreachable) else message(failure, R.string.fuel_refused)
            }
        }
    }
}

/** The receipt's printed local time as the server's UTC `NaiveDateTime`; now when the receipt had none. */
internal fun recordedAtUtc(printedLocal: String?, zone: TimeZone = TimeZone.getDefault()): String {
    val parsed = printedLocal?.let {
        runCatching { SimpleDateFormat("yyyy-MM-dd'T'HH:mm:ss", Locale.US).apply { timeZone = zone }.parse(it) }.getOrNull()
    }
    // A printed time in the future (a misread digit) cannot be a past fuelling: stamp it now instead.
    val instant = parsed?.takeIf { it.time <= System.currentTimeMillis() } ?: Date()
    return Instants.nowUtc(instant)
}
