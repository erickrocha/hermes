package com.erocha.hermes.client.driver

import android.text.Editable
import android.text.InputType
import android.text.TextWatcher
import android.widget.Button
import android.widget.CheckBox
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.RadioButton
import android.widget.RadioGroup
import com.erocha.hermes.client.shared.ANSWER_CONFORMING
import com.erocha.hermes.client.shared.ANSWER_NON_CONFORMING
import com.erocha.hermes.client.shared.CHECKLIST_DEPARTURE
import com.erocha.hermes.client.shared.CHECKLIST_RETURN
import com.erocha.hermes.client.shared.ExtraTrip
import com.erocha.hermes.client.shared.FuelEntry
import com.erocha.hermes.client.shared.VehicleTracking
import com.erocha.hermes.client.shared.buildFuelReceipt
import com.erocha.hermes.client.shared.canSubmitReceipt
import com.erocha.hermes.client.shared.ChecklistTemplate
import com.erocha.hermes.client.shared.Vehicle
import com.erocha.hermes.client.shared.VehicleAction
import com.erocha.hermes.client.shared.buildChecklistRun
import com.erocha.hermes.client.shared.canSubmitChecklist
import com.erocha.hermes.client.shared.defaultTemplate
import com.erocha.hermes.client.shared.parseOdometer
import com.erocha.hermes.client.shared.vehicleAction

/**
 * The driver's screens (C-031, first cut): the assigned vehicle and the departure/return checklist. The server decides
 * scope, kilometre and possession (`SR-CA-03-01`); this only presents and collects. State lives in memory only
 * (`SR-CA-03-05`) and survives rotation because the activity is not recreated.
 */
internal class FleetContent(private val host: FleetHost, private val container: LinearLayout) {
    private data class VehicleRow(val vehicle: Vehicle, val action: VehicleAction)

    private class Form(
        val row: VehicleRow,
        val template: ChecklistTemplate,
        val answers: MutableMap<String, String> = mutableMapOf(),
        val observations: MutableMap<String, String> = mutableMapOf(),
        var odometer: String = "",
        var notes: String = "",
        var submitting: Boolean = false,
        /** Set after a send with no answer: the server may or may not have recorded it. */
        var needsCheck: Boolean = false,
        var message: String? = null,
    )

    private class Receipt(
        val vehicle: Vehicle,
        var prefix: String = "",
        var liters: String = "",
        var value: String = "",
        var odometer: String = "",
        var station: String = "",
        var fullTank: Boolean = false,
        var submitting: Boolean = false,
        /** Set after `OdometerOutsideWindow`: the driver may confirm the reading and send anyway. */
        var canOverride: Boolean = false,
        var message: String? = null,
    )

    private var rows: List<VehicleRow>? = null
    private var form: Form? = null
    private var receipt: Receipt? = null
    private var homeMessage: String? = null

    fun start() = when {
        form != null -> showForm()
        receipt != null -> showReceipt()
        else -> loadHome()
    }

    // ---- Home: the assigned vehicle(s) ----

    private fun loadHome(message: String? = null) {
        form = null
        receipt = null
        homeMessage = message
        showLoading()
        host.run({ api, token ->
            api.vehicles(token).map { VehicleRow(it, vehicleAction(api.possession(token, it.uuid), host.userUuid)) }
        }) { result ->
            result.onSuccess { rows = it; showHome() }
                .onFailure { rows = null; showError(describe(it, R.string.load_failed)) { loadHome() } }
        }
    }

    private fun showLoading() {
        container.removeAllViews()
        container.addView(host.text(host.context.getString(R.string.loading), R.style.TextAppearance_Hermes_Body), host.spacing())
    }

    private fun showError(message: String, retry: () -> Unit) {
        container.removeAllViews()
        container.addView(host.text(message, R.style.TextAppearance_Hermes_Error), host.spacing())
        container.addView(button(R.string.refresh_button) { retry() }, host.spacing())
    }

    private fun showHome() {
        container.removeAllViews()
        val context = host.context
        container.addView(host.text(context.getString(R.string.my_vehicle_title), R.style.TextAppearance_Hermes_Title), host.spacing())
        homeMessage?.let { container.addView(host.text(it, R.style.TextAppearance_Hermes_Body), host.spacing()) }
        val list = rows.orEmpty()
        if (list.isEmpty()) container.addView(host.text(context.getString(R.string.no_vehicle), R.style.TextAppearance_Hermes_Body), host.spacing())
        list.forEach { row ->
            val v = row.vehicle
            container.addView(host.text(context.getString(R.string.vehicle_line, v.plate, v.model), R.style.TextAppearance_Hermes_Title), host.spacing())
            container.addView(host.text(context.getString(R.string.vehicle_status, statusLabel(v.status)), R.style.TextAppearance_Hermes_Body))
            val km = v.odometerKm?.let { context.getString(R.string.vehicle_odometer, formatKm(it)) } ?: context.getString(R.string.vehicle_odometer_unknown)
            container.addView(host.text(km, R.style.TextAppearance_Hermes_Body))
            when (row.action) {
                VehicleAction.Depart -> container.addView(button(R.string.register_departure) { openForm(row) }, host.spacing())
                is VehicleAction.Return -> container.addView(button(R.string.register_return) { openForm(row) }, host.spacing())
                VehicleAction.HeldByAnother -> container.addView(host.text(context.getString(R.string.held_by_another), R.style.TextAppearance_Hermes_Error), host.spacing())
            }
            container.addView(button(R.string.view_position) { showPosition(v) }, host.spacing())
            container.addView(button(R.string.register_fuel) { receipt = Receipt(v); showReceipt() }, host.spacing())
        }
        container.addView(button(R.string.my_trips) { showTrips() }, host.spacing())
        container.addView(button(R.string.my_fuel_entries) { showFuelEntries() }, host.spacing())
        container.addView(button(R.string.refresh_button) { loadHome() }, host.spacing())
    }

    // ---- Checklist form ----

    private fun openForm(row: VehicleRow) {
        val type = if (row.action is VehicleAction.Return) CHECKLIST_RETURN else CHECKLIST_DEPARTURE
        showLoading()
        host.run({ api, token -> defaultTemplate(api.templates(token), type)?.let { api.template(token, it.uuid) } }) { result ->
            result.onSuccess { template ->
                if (template == null) {
                    showError(host.context.getString(R.string.no_default_template)) { loadHome() }
                } else {
                    form = Form(row, template)
                    showForm()
                }
            }.onFailure { showError(describe(it, R.string.load_failed)) { loadHome() } }
        }
    }

    private fun showForm() {
        val current = form ?: return loadHome()
        val context = host.context
        container.removeAllViews()
        val titleRes = if (current.template.checklistType == CHECKLIST_RETURN) R.string.return_form_title else R.string.departure_form_title
        container.addView(host.text(context.getString(titleRes), R.style.TextAppearance_Hermes_Title), host.spacing())
        container.addView(host.text(context.getString(R.string.vehicle_line, current.row.vehicle.plate, current.row.vehicle.model), R.style.TextAppearance_Hermes_Body))

        val submit = Button(context)
        fun refreshSubmit() {
            val ready = canSubmitChecklist(current.odometer, current.template.items, current.answers)
            submit.isEnabled = ready && !current.submitting
            submit.text = context.getString(if (current.submitting) R.string.submitting else if (current.needsCheck) R.string.check_button else R.string.submit_button)
        }

        container.addView(editText(R.string.odometer_hint, current.odometer, decimal = true) { current.odometer = it; refreshSubmit() }, host.spacing())
        current.template.items.forEach { item ->
            container.addView(host.text(item.description, R.style.TextAppearance_Hermes_Body), host.spacing())
            val group = RadioGroup(context).apply { orientation = RadioGroup.HORIZONTAL }
            listOf(ANSWER_CONFORMING to R.string.answer_conforming, ANSWER_NON_CONFORMING to R.string.answer_non_conforming).forEach { (status, label) ->
                group.addView(RadioButton(context).apply {
                    id = android.view.View.generateViewId()
                    text = context.getString(label)
                    isChecked = current.answers[item.uuid] == status
                    setOnClickListener { current.answers[item.uuid] = status; refreshSubmit() }
                })
            }
            container.addView(group)
            container.addView(editText(R.string.observation_hint, current.observations[item.uuid].orEmpty()) { current.observations[item.uuid] = it })
        }
        container.addView(editText(R.string.notes_hint, current.notes) { current.notes = it }, host.spacing())

        current.message?.let { container.addView(host.text(it, R.style.TextAppearance_Hermes_Error), host.spacing()) }
        submit.setOnClickListener { if (current.needsCheck) check(current) else submit(current) }
        refreshSubmit()
        container.addView(submit, host.spacing())
        container.addView(button(R.string.cancel_button) { loadHome() }, host.spacing())
    }

    private fun submit(current: Form) {
        val km = parseOdometer(current.odometer) ?: return
        val run = buildChecklistRun(current.template, host.userUuid, current.row.vehicle.uuid, current.row.action, km, current.answers, current.observations, current.notes)
        current.submitting = true
        current.message = null
        showForm()
        host.run({ api, token -> api.submitRun(token, run) }) { result ->
            current.submitting = false
            result.onSuccess { loadHome(host.context.getString(R.string.submitted)) }
                .onFailure { failure ->
                    if (failure is FleetUnreachable) {
                        current.needsCheck = true
                        current.message = host.context.getString(R.string.submit_unreachable)
                    } else {
                        current.message = describe(failure, R.string.submit_refused)
                    }
                    if (form === current) showForm()
                }
        }
    }

    /** After an unanswered send: ask the server whether it was recorded before allowing another (`SR-CA-03-03`). */
    private fun check(current: Form) {
        current.submitting = true
        showForm()
        host.run({ api, token -> vehicleAction(api.possession(token, current.row.vehicle.uuid), host.userUuid) }) { result ->
            current.submitting = false
            result.onSuccess { now ->
                if (now != current.row.action) {
                    loadHome(host.context.getString(R.string.already_registered))
                } else {
                    current.needsCheck = false
                    current.message = host.context.getString(R.string.not_yet_registered)
                    if (form === current) showForm()
                }
            }.onFailure {
                current.message = host.context.getString(R.string.submit_unreachable)
                if (form === current) showForm()
            }
        }
    }

    // ---- Position (SR-CA-03-12) ----

    private fun showPosition(vehicle: Vehicle) {
        showLoading()
        host.run({ api, token -> api.tracking(token, vehicle.uuid) }) { result ->
            result.onSuccess { renderPosition(vehicle, it) }.onFailure { failure ->
                val message = when {
                    failure is FleetRefused && failure.errorKey == "TrackerNotLinked" -> host.context.getString(R.string.tracker_not_linked)
                    failure is FleetRefused && failure.status == 503 -> host.context.getString(R.string.tracking_unavailable)
                    else -> describe(failure, R.string.load_failed)
                }
                showError(message) { loadHome() }
            }
        }
    }

    private fun renderPosition(vehicle: Vehicle, tracking: VehicleTracking) {
        val context = host.context
        container.removeAllViews()
        container.addView(host.text(context.getString(R.string.position_title), R.style.TextAppearance_Hermes_Title), host.spacing())
        container.addView(host.text(context.getString(R.string.vehicle_line, vehicle.plate, vehicle.model), R.style.TextAppearance_Hermes_Body))
        val point = tracking.position
        if (point == null) {
            container.addView(host.text(context.getString(R.string.position_none), R.style.TextAppearance_Hermes_Body), host.spacing())
        } else {
            val label = if (tracking.positionLive) R.string.position_live else R.string.position_stale
            // A reading that is not live is never passed off as live (HRMS-929).
            container.addView(host.text(context.getString(label), if (tracking.positionLive) R.style.TextAppearance_Hermes_Title else R.style.TextAppearance_Hermes_Error), host.spacing())
            container.addView(host.text(context.getString(R.string.position_coordinates, point.latitude.toString(), point.longitude.toString()), R.style.TextAppearance_Hermes_Body))
            container.addView(host.text(context.getString(R.string.position_updated, Instants.toLocal(tracking.positionReportedAt ?: tracking.reportedAt)), R.style.TextAppearance_Hermes_Body))
        }
        container.addView(host.text(context.getString(R.string.ignition_line, ignitionLabel(tracking.ignition)), R.style.TextAppearance_Hermes_Body))
        if (!tracking.trustworthy) container.addView(host.text(context.getString(R.string.ignition_untrusted), R.style.TextAppearance_Hermes_Error))
        point?.let {
            container.addView(button(R.string.open_map) {
                if (!host.openMap(it.latitude, it.longitude)) container.addView(host.text(context.getString(R.string.no_map_app), R.style.TextAppearance_Hermes_Error), host.spacing())
            }, host.spacing())
        }
        container.addView(button(R.string.refresh_position) { showPosition(vehicle) }, host.spacing())
        container.addView(button(R.string.back_button) { loadHome() }, host.spacing())
    }

    private fun ignitionLabel(state: String): String = host.context.getString(
        when (state) { "On" -> R.string.ignition_On; "Off" -> R.string.ignition_Off; else -> R.string.ignition_Unknown },
    )

    // ---- Fuel receipt (SR-CA-03-13/14) ----

    private fun showReceipt() {
        val current = receipt ?: return loadHome()
        val context = host.context
        container.removeAllViews()
        container.addView(host.text(context.getString(R.string.fuel_form_title), R.style.TextAppearance_Hermes_Title), host.spacing())
        container.addView(host.text(context.getString(R.string.vehicle_line, current.vehicle.plate, current.vehicle.model), R.style.TextAppearance_Hermes_Body))

        val send = Button(context)
        val sendAnyway = Button(context)
        fun refreshSend() {
            val ready = canSubmitReceipt(current.prefix, current.liters, current.value, current.odometer) && !current.submitting
            send.isEnabled = ready
            send.text = context.getString(if (current.submitting) R.string.submitting else R.string.submit_button)
            sendAnyway.isEnabled = ready
        }
        container.addView(editText(R.string.prefix_hint, current.prefix) { current.prefix = it; refreshSend() }, host.spacing())
        container.addView(editText(R.string.liters_hint, current.liters, decimal = true) { current.liters = it; refreshSend() }, host.spacing())
        container.addView(editText(R.string.value_hint, current.value, decimal = true) { current.value = it; refreshSend() }, host.spacing())
        container.addView(editText(R.string.fuel_odometer_hint, current.odometer, decimal = true) { current.odometer = it; refreshSend() }, host.spacing())
        container.addView(editText(R.string.station_hint, current.station) { current.station = it }, host.spacing())
        container.addView(CheckBox(context).apply {
            text = context.getString(R.string.full_tank)
            isChecked = current.fullTank
            setOnClickListener { current.fullTank = isChecked }
        }, host.spacing())
        current.message?.let { container.addView(host.text(it, R.style.TextAppearance_Hermes_Error), host.spacing()) }
        send.setOnClickListener { submitReceipt(current, override = false) }
        container.addView(send, host.spacing())
        if (current.canOverride) {
            sendAnyway.text = context.getString(R.string.send_anyway)
            sendAnyway.setOnClickListener { submitReceipt(current, override = true) }
            container.addView(sendAnyway, host.spacing())
        }
        refreshSend()
        container.addView(button(R.string.cancel_button) { loadHome() }, host.spacing())
    }

    private fun submitReceipt(current: Receipt, override: Boolean) {
        val request = buildFuelReceipt(
            current.vehicle.uuid, current.prefix, Instants.nowUtc(), current.liters, current.value, current.odometer, current.station, current.fullTank, override,
        ) ?: return
        current.submitting = true
        current.message = null
        showReceipt()
        host.run({ api, token -> api.submitReceipt(token, request) }) { result ->
            current.submitting = false
            result.onSuccess {
                loadHome(host.context.getString(if (it.alreadyRecorded) R.string.fuel_already_registered else R.string.fuel_registered))
            }.onFailure { failure ->
                current.canOverride = failure is FleetRefused && failure.errorKey == "OdometerOutsideWindow"
                current.message = if (failure is FleetUnreachable) host.context.getString(R.string.fuel_unreachable) else describe(failure, R.string.fuel_refused)
                if (receipt === current) showReceipt()
            }
        }
    }

    private fun showFuelEntries() {
        showLoading()
        host.run({ api, token -> api.fuelEntries(token) }) { result ->
            result.onSuccess { renderFuelEntries(it) }.onFailure { showError(describe(it, R.string.load_failed)) { loadHome() } }
        }
    }

    private fun renderFuelEntries(entries: List<FuelEntry>) {
        val context = host.context
        container.removeAllViews()
        container.addView(host.text(context.getString(R.string.my_fuel_entries), R.style.TextAppearance_Hermes_Title), host.spacing())
        if (entries.isEmpty()) container.addView(host.text(context.getString(R.string.no_fuel_entries), R.style.TextAppearance_Hermes_Body), host.spacing())
        entries.forEach { entry ->
            val liters = String.format(java.util.Locale("pt", "BR"), "%.1f", entry.volumeLiters)
            container.addView(host.text(context.getString(R.string.fuel_line, Instants.toLocal(entry.recordedAt), liters, Instants.money(entry.valueCents)), R.style.TextAppearance_Hermes_Body), host.spacing())
            entry.station?.let { container.addView(host.text(context.getString(R.string.fuel_station_line, it), R.style.TextAppearance_Hermes_Body)) }
        }
        container.addView(button(R.string.back_button) { loadHome() }, host.spacing())
    }

    // ---- Trips (SR-CA-03-11): the server returns only the ones that name the driver ----

    private fun showTrips() {
        showLoading()
        host.run({ api, token -> api.trips(token) }) { result ->
            result.onSuccess { renderTrips(it) }.onFailure { showError(describe(it, R.string.load_failed)) { loadHome() } }
        }
    }

    private fun renderTrips(trips: List<ExtraTrip>) {
        val context = host.context
        container.removeAllViews()
        container.addView(host.text(context.getString(R.string.trips_title), R.style.TextAppearance_Hermes_Title), host.spacing())
        if (trips.isEmpty()) container.addView(host.text(context.getString(R.string.no_trips), R.style.TextAppearance_Hermes_Body), host.spacing())
        trips.forEach { trip ->
            container.addView(host.text(context.getString(R.string.trip_when, Instants.date(trip.tripDate), Instants.time(trip.startTime)).trim(), R.style.TextAppearance_Hermes_Title), host.spacing())
            if (!trip.originCity.isNullOrBlank() || !trip.destination.isNullOrBlank()) {
                container.addView(host.text(context.getString(R.string.trip_route, trip.originCity.orEmpty(), trip.destination.orEmpty()), R.style.TextAppearance_Hermes_Body))
            }
            trip.stops?.takeIf { it.isNotBlank() }?.let { container.addView(host.text(context.getString(R.string.trip_stops, it), R.style.TextAppearance_Hermes_Body)) }
            container.addView(host.text(context.getString(R.string.trip_code, trip.orderCode, tripStatus(trip.status)), R.style.TextAppearance_Hermes_Body))
        }
        container.addView(button(R.string.back_button) { loadHome() }, host.spacing())
    }

    private fun tripStatus(status: String): String = when (status) {
        "Scheduled" -> host.context.getString(R.string.trip_status_Scheduled)
        "Conflict" -> host.context.getString(R.string.trip_status_Conflict)
        "PendingSchedule" -> host.context.getString(R.string.trip_status_PendingSchedule)
        "Cancelled" -> host.context.getString(R.string.trip_status_Cancelled)
        else -> status
    }

    // ---- helpers ----

    private fun describe(failure: Throwable, fallback: Int): String =
        (failure as? FleetRefused)?.reason ?: host.context.getString(fallback)

    private fun button(label: Int, onClick: () -> Unit) = Button(host.context).apply {
        text = host.context.getString(label)
        setOnClickListener { onClick() }
    }

    private fun editText(hint: Int, value: String, decimal: Boolean = false, onChange: (String) -> Unit) = EditText(host.context).apply {
        this.hint = host.context.getString(hint)
        contentDescription = host.context.getString(hint)
        inputType = if (decimal) InputType.TYPE_CLASS_NUMBER or InputType.TYPE_NUMBER_FLAG_DECIMAL else InputType.TYPE_CLASS_TEXT
        setText(value)
        addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(s: CharSequence?, start: Int, count: Int, after: Int) = Unit
            override fun onTextChanged(s: CharSequence?, start: Int, before: Int, count: Int) = Unit
            override fun afterTextChanged(s: Editable?) = onChange(s?.toString().orEmpty())
        })
    }

    private fun statusLabel(status: String): String {
        val res = when (status) {
            "Active" -> R.string.status_Active
            "Maintenance" -> R.string.status_Maintenance
            "Transit" -> R.string.status_Transit
            "Reserved" -> R.string.status_Reserved
            "Inactive" -> R.string.status_Inactive
            else -> return status
        }
        return host.context.getString(res)
    }

    private fun formatKm(km: Double): String = if (km % 1.0 == 0.0) km.toLong().toString() else km.toString()
}
