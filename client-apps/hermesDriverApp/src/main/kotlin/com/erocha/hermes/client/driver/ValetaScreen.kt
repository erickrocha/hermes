package com.erocha.hermes.client.driver

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateListOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.erocha.hermes.client.shared.InspectionAnswer
import com.erocha.hermes.client.shared.InspectionModel
import com.erocha.hermes.client.shared.TechnicalInspectionRequest
import com.erocha.hermes.client.shared.TechnicalInspectionResult
import com.erocha.hermes.client.shared.Vehicle
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/** One checklist line: [conforming] stays null until the mechanic answers it. */
internal class Answer(val description: String) {
    var conforming by mutableStateOf<Boolean?>(null)
    var observation by mutableStateOf("")
}

internal enum class ValetaStep { VEHICLE, MODEL, CHECKLIST, DONE }

/** The valeta inspection flow: vehicle, then checklist model, then the answers, then the outcome. */
internal class ValetaState(private val host: FleetHost) {
    var step by mutableStateOf(ValetaStep.VEHICLE)
    var vehicles by mutableStateOf<List<Vehicle>?>(null)
    var models by mutableStateOf<List<InspectionModel>?>(null)
    var error by mutableStateOf<String?>(null)
    var vehicle by mutableStateOf<Vehicle?>(null)
    var model by mutableStateOf<InspectionModel?>(null)
    val answers = mutableStateListOf<Answer>()
    var odometer by mutableStateOf("")
    var notes by mutableStateOf("")
    var sending by mutableStateOf(false)
    var result by mutableStateOf<TechnicalInspectionResult?>(null)

    val canGoBack get() = step == ValetaStep.MODEL || step == ValetaStep.CHECKLIST

    val answered get() = answers.count { it.conforming != null }
    val canSubmit get() = !sending && answers.isNotEmpty() && answered == answers.size

    fun start() {
        if (vehicles != null && models != null) return
        error = null
        host.run({ api, token -> api.vehicles(token) }) { r ->
            r.onSuccess { vehicles = it }.onFailure { error = failureMessage(host, it, R.string.valeta_loading_failed) }
        }
        host.runWorkshop({ api, token -> api.inspectionModels(token) }) { r ->
            r.onSuccess { models = it }.onFailure { error = failureMessage(host, it, R.string.valeta_loading_failed) }
        }
    }

    fun pick(chosen: Vehicle) {
        vehicle = chosen
        val available = models.orEmpty()
        // One checklist needs no choosing.
        if (available.size == 1) choose(available.single()) else step = ValetaStep.MODEL
    }

    fun choose(chosen: InspectionModel) {
        model = chosen
        answers.clear()
        answers.addAll(chosen.items.map(::Answer))
        odometer = ""
        notes = ""
        error = null
        step = ValetaStep.CHECKLIST
    }

    fun back() {
        error = null
        step = if (step == ValetaStep.CHECKLIST && (models?.size ?: 0) > 1) ValetaStep.MODEL else ValetaStep.VEHICLE
    }

    fun submit() {
        val chosenVehicle = vehicle ?: return
        val chosenModel = model ?: return
        sending = true
        error = null
        val request = TechnicalInspectionRequest(
            vehicleUuid = chosenVehicle.uuid,
            inspectionModel = chosenModel.name,
            inspectedAt = SimpleDateFormat("yyyy-MM-dd", Locale.US).format(Date()),
            odometerKm = odometer.replace(',', '.').toDoubleOrNull(),
            observation = notes.trim().ifEmpty { null },
            items = answers.map {
                InspectionAnswer(it.description, it.conforming == true, it.observation.trim().ifEmpty { null })
            },
        )
        host.runWorkshop({ api, token -> api.submitInspection(token, request) }) { r ->
            sending = false
            r.onSuccess { result = it; step = ValetaStep.DONE }
                .onFailure { error = failureMessage(host, it, R.string.valeta_submit_failed) }
        }
    }

    fun reset() {
        vehicle = null
        model = null
        answers.clear()
        result = null
        error = null
        step = ValetaStep.VEHICLE
    }
}

@Composable
internal fun ValetaTab(state: ValetaState) {
    Column(modifier = Modifier.fillMaxSize()) {
        state.error?.let { ErrorBanner(it) }
        when (state.step) {
            ValetaStep.VEHICLE -> VehicleStep(state)
            ValetaStep.MODEL -> ModelStep(state)
            ValetaStep.CHECKLIST -> ChecklistStep(state)
            ValetaStep.DONE -> DoneStep(state)
        }
    }
}

@Composable
private fun StepTitle(text: String) {
    Text(text, color = token(R.attr.hermesTextPrimary), fontSize = 18.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(horizontal = 20.dp, vertical = 12.dp))
}

@Composable
private fun VehicleStep(state: ValetaState) {
    var query by remember { mutableStateOf("") }
    val shown = state.vehicles.orEmpty().filter {
        query.isBlank() || (it.prefix ?: "").contains(query.trim(), ignoreCase = true) || it.plate.contains(query.trim(), ignoreCase = true)
    }
    StepTitle(stringResource(R.string.valeta_pick_vehicle))
    OutlinedTextField(
        value = query, onValueChange = { query = it }, singleLine = true,
        placeholder = { Text(stringResource(R.string.garage_search_hint)) },
        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp),
    )
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.spacedBy(8.dp),
        contentPadding = PaddingValues(16.dp),
    ) {
        items(shown, key = { it.uuid }) { vehicle ->
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(token(R.attr.hermesSurface), RoundedCornerShape(14.dp))
                    .border(BorderStroke(1.dp, token(R.attr.hermesOutline)), RoundedCornerShape(14.dp))
                    .clickable { state.pick(vehicle) }
                    .padding(16.dp),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(14.dp),
            ) {
                Text(vehicle.prefix ?: vehicle.plate, color = token(R.attr.hermesTextPrimary), fontSize = 22.sp, fontWeight = FontWeight.Bold)
                Text(vehicle.plate, color = token(R.attr.hermesTextSecondary), fontSize = 15.sp)
            }
        }
    }
}

@Composable
private fun ModelStep(state: ValetaState) {
    StepTitle(stringResource(R.string.valeta_pick_model))
    val available = state.models.orEmpty()
    if (available.isEmpty()) {
        Text(stringResource(R.string.valeta_no_models), color = token(R.attr.hermesTextSecondary), modifier = Modifier.padding(20.dp))
        return
    }
    LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp), contentPadding = PaddingValues(16.dp)) {
        items(available, key = { it.uuid ?: it.name }) { model ->
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .background(token(R.attr.hermesSurface), RoundedCornerShape(14.dp))
                    .border(BorderStroke(1.dp, token(R.attr.hermesOutline)), RoundedCornerShape(14.dp))
                    .clickable { state.choose(model) }
                    .padding(16.dp),
            ) {
                Text(model.name, color = token(R.attr.hermesTextPrimary), fontSize = 18.sp, fontWeight = FontWeight.SemiBold)
                Text("${model.items.size} itens", color = token(R.attr.hermesTextSecondary), fontSize = 14.sp)
            }
        }
    }
}

@Composable
private fun ColumnScope.ChecklistStep(state: ValetaState) {
    val vehicle = state.vehicle
    Row(modifier = Modifier.fillMaxWidth().padding(start = 8.dp, end = 20.dp), verticalAlignment = Alignment.CenterVertically) {
        TextButton(onClick = { state.back() }) { Text("‹ ${stringResource(R.string.garage_back)}", color = token(R.attr.hermesAccent), fontSize = 16.sp) }
        Spacer(Modifier.weight(1f))
        Text(vehicle?.prefix ?: vehicle?.plate.orEmpty(), color = token(R.attr.hermesTextPrimary), fontSize = 26.sp, fontWeight = FontWeight.Bold)
    }
    Column(modifier = Modifier.padding(horizontal = 20.dp, vertical = 4.dp)) {
        Text(stringResource(R.string.valeta_answered, state.answered, state.answers.size), color = token(R.attr.hermesTextSecondary), fontSize = 14.sp)
        Spacer(Modifier.height(6.dp))
        LinearProgressIndicator(
            progress = { if (state.answers.isEmpty()) 0f else state.answered / state.answers.size.toFloat() },
            modifier = Modifier.fillMaxWidth().height(6.dp),
            color = token(R.attr.hermesOk),
            trackColor = token(R.attr.hermesOutline),
        )
    }
    LazyColumn(
        modifier = Modifier.weight(1f),
        verticalArrangement = Arrangement.spacedBy(10.dp),
        contentPadding = PaddingValues(16.dp),
    ) {
        items(state.answers, key = { it.description }) { answer -> AnswerCard(answer) }
        item {
            OutlinedTextField(
                value = state.odometer, onValueChange = { state.odometer = it }, singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                placeholder = { Text(stringResource(R.string.valeta_odometer_hint)) }, modifier = Modifier.fillMaxWidth(),
            )
        }
        item {
            OutlinedTextField(
                value = state.notes, onValueChange = { state.notes = it },
                placeholder = { Text(stringResource(R.string.valeta_general_hint)) }, modifier = Modifier.fillMaxWidth(),
            )
        }
    }
    Button(
        onClick = { state.submit() },
        enabled = state.canSubmit,
        modifier = Modifier.fillMaxWidth().padding(16.dp).height(56.dp),
        shape = RoundedCornerShape(16.dp),
        colors = ButtonDefaults.buttonColors(containerColor = token(R.attr.hermesAccent), contentColor = Color.White),
    ) { Text(stringResource(if (state.sending) R.string.valeta_sending else R.string.valeta_submit), fontSize = 17.sp, fontWeight = FontWeight.SemiBold) }
}

@Composable
private fun AnswerCard(answer: Answer) {
    val bad = answer.conforming == false
    val outline = if (bad) token(R.attr.hermesError) else token(R.attr.hermesOutline)
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(token(R.attr.hermesSurface), RoundedCornerShape(16.dp))
            .border(BorderStroke(1.dp, outline), RoundedCornerShape(16.dp))
            .padding(16.dp),
    ) {
        Text(answer.description, color = token(R.attr.hermesTextPrimary), fontSize = 17.sp, fontWeight = FontWeight.SemiBold)
        Spacer(Modifier.height(10.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            ChoiceButton(stringResource(R.string.valeta_conforming), answer.conforming == true, token(R.attr.hermesOk), Modifier.weight(1f)) { answer.conforming = true }
            ChoiceButton(stringResource(R.string.valeta_not_conforming), bad, token(R.attr.hermesError), Modifier.weight(1f)) { answer.conforming = false }
        }
        if (bad) {
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                value = answer.observation, onValueChange = { answer.observation = it },
                placeholder = { Text(stringResource(R.string.valeta_observation_hint)) }, modifier = Modifier.fillMaxWidth(),
            )
        }
    }
}

@Composable
private fun DoneStep(state: ValetaState) {
    val opened = state.result?.workOrderOpened == true
    Column(modifier = Modifier.fillMaxSize().padding(24.dp), verticalArrangement = Arrangement.Center, horizontalAlignment = Alignment.CenterHorizontally) {
        Text("✓", color = token(R.attr.hermesOk), fontSize = 64.sp, fontWeight = FontWeight.Bold)
        Text(stringResource(R.string.valeta_done_title), color = token(R.attr.hermesTextPrimary), fontSize = 22.sp, fontWeight = FontWeight.Bold)
        Spacer(Modifier.height(8.dp))
        Text(
            stringResource(if (opened) R.string.valeta_done_work_order else R.string.valeta_done_clean),
            color = token(if (opened) R.attr.hermesWarning else R.attr.hermesTextSecondary),
            fontSize = 16.sp,
        )
        Spacer(Modifier.height(24.dp))
        Button(
            onClick = { state.reset() }, modifier = Modifier.fillMaxWidth().height(56.dp), shape = RoundedCornerShape(16.dp),
            colors = ButtonDefaults.buttonColors(containerColor = token(R.attr.hermesAccent), contentColor = Color.White),
        ) { Text(stringResource(R.string.valeta_another), fontSize = 17.sp, fontWeight = FontWeight.SemiBold) }
    }
}
