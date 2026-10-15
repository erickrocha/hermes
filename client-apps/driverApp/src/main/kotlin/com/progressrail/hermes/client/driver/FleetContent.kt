package com.progressrail.hermes.client.driver

import android.text.Editable
import android.text.InputType
import android.text.TextWatcher
import android.widget.Button
import android.widget.EditText
import android.widget.LinearLayout
import android.widget.RadioButton
import android.widget.RadioGroup
import com.progressrail.hermes.client.shared.ANSWER_CONFORMING
import com.progressrail.hermes.client.shared.ANSWER_NON_CONFORMING
import com.progressrail.hermes.client.shared.CHECKLIST_DEPARTURE
import com.progressrail.hermes.client.shared.CHECKLIST_RETURN
import com.progressrail.hermes.client.shared.ChecklistTemplate
import com.progressrail.hermes.client.shared.Vehicle
import com.progressrail.hermes.client.shared.VehicleAction
import com.progressrail.hermes.client.shared.buildChecklistRun
import com.progressrail.hermes.client.shared.canSubmitChecklist
import com.progressrail.hermes.client.shared.defaultTemplate
import com.progressrail.hermes.client.shared.parseOdometer
import com.progressrail.hermes.client.shared.vehicleAction

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

    private var rows: List<VehicleRow>? = null
    private var form: Form? = null
    private var homeMessage: String? = null

    fun start() = if (form != null) showForm() else loadHome()

    // ---- Home: the assigned vehicle(s) ----

    private fun loadHome(message: String? = null) {
        form = null
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
        }
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
