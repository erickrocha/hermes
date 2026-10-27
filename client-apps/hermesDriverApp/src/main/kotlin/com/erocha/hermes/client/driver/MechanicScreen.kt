package com.erocha.hermes.client.driver

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.material3.NavigationBar
import androidx.compose.material3.NavigationBarItem
import androidx.compose.material3.NavigationBarItemDefaults
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.erocha.hermes.client.shared.GarageAttendance
import com.erocha.hermes.client.shared.GarageQueueEntry
import com.erocha.hermes.client.shared.GarageService
import com.erocha.hermes.client.shared.STATE_NOT_DONE
import com.erocha.hermes.client.shared.STATE_NOT_NEEDED
import com.erocha.hermes.client.shared.STATE_PENDING
import com.erocha.hermes.client.shared.STATE_PERFORMED
import com.erocha.hermes.client.shared.Vehicle
import com.erocha.hermes.client.shared.themeColor
import kotlinx.coroutines.delay

private const val POLL_MILLIS = 20_000L

/** The mechanic's screen state; every call goes through the host, which renews the session and reports refusals. */
internal class MechanicState(private val host: FleetHost) {
    var queue by mutableStateOf<List<GarageQueueEntry>?>(null)
    var refreshing by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)
    var group by mutableStateOf(GarageRules.ALL)

    var open by mutableStateOf<GarageAttendance?>(null)
    var openPrefix by mutableStateOf("")
    var saving by mutableStateOf<String?>(null)

    var picking by mutableStateOf(false)
    var vehicles by mutableStateOf<List<Vehicle>?>(null)

    private fun message(failure: Throwable, fallback: Int) = failureMessage(host, failure, fallback)

    fun refresh(silent: Boolean = false) {
        if (!silent) refreshing = true
        host.runGarage({ api, token -> api.queue(token) }) { result ->
            refreshing = false
            result.onSuccess { queue = it; if (open == null) error = null }
                .onFailure { if (!silent || queue == null) error = message(it, R.string.garage_loading_failed) }
        }
        open?.uuid?.let { uuid ->
            host.runGarage({ api, token -> api.attendance(token, uuid) }) { result ->
                result.onSuccess { if (open?.uuid == it.uuid && saving == null) open = it }
            }
        }
    }

    fun show(entry: GarageQueueEntry) {
        val uuid = entry.attendanceUuid ?: return
        openPrefix = entry.prefix
        error = null
        host.runGarage({ api, token -> api.attendance(token, uuid) }) { result ->
            result.onSuccess { open = it }.onFailure { error = message(it, R.string.garage_loading_failed) }
        }
    }

    fun close() {
        open = null
        error = null
        refresh(silent = true)
    }

    fun mark(service: GarageService, state: String) {
        val attendance = open ?: return
        val attendanceUuid = attendance.uuid ?: return
        val serviceUuid = service.uuid ?: return
        saving = serviceUuid
        error = null
        host.runGarage({ api, token -> api.mark(token, attendanceUuid, serviceUuid, state) }) { result ->
            saving = null
            result.onSuccess { open = it }.onFailure { error = message(it, R.string.garage_mark_failed) }
        }
    }

    fun startPicking() {
        picking = true
        if (vehicles == null) {
            host.run({ api, token -> api.vehicles(token) }) { result ->
                result.onSuccess { vehicles = it }.onFailure { picking = false; error = message(it, R.string.garage_loading_failed) }
            }
        }
    }

    fun openTriage(vehicle: Vehicle) {
        picking = false
        openPrefix = vehicle.prefix ?: vehicle.plate
        host.runGarage({ api, token -> api.open(token, vehicle.uuid) }) { result ->
            result.onSuccess { open = it; refresh(silent = true) }.onFailure { error = message(it, R.string.garage_loading_failed) }
        }
    }
}

@Composable
internal fun MechanicApp(host: FleetHost, userName: String, themeLabel: String, onTheme: () -> Unit, onSignOut: () -> Unit) {
    HermesMaterialTheme { MechanicBody(host, userName, themeLabel, onTheme, onSignOut) }
}

@Composable
private fun MechanicBody(host: FleetHost, userName: String, themeLabel: String, onTheme: () -> Unit, onSignOut: () -> Unit) {
    val garage = remember { MechanicState(host) }
    val valeta = remember { ValetaState(host) }
    val orders = remember { MaintenanceState(host) }
    var tab by rememberSaveable { mutableIntStateOf(0) }
    LaunchedEffect(Unit) {
        garage.refresh()
        while (true) {
            delay(POLL_MILLIS)
            if (garage.saving == null) garage.refresh(silent = true)
            if (tab == 2 && orders.busy == null) orders.refresh(silent = true)
        }
    }
    LaunchedEffect(tab) {
        if (tab == 1) valeta.start()
        if (tab == 2) orders.refresh()
    }
    BackHandler(enabled = tab == 0 && garage.open != null) { garage.close() }
    BackHandler(enabled = tab == 1 && valeta.canGoBack) { valeta.back() }
    BackHandler(enabled = tab == 2 && orders.open != null) { orders.close() }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(token(R.attr.hermesScreenBackground))
            .statusBarsPadding()
            .navigationBarsPadding(),
    ) {
        val attendance = garage.open
        if (tab == 0 && attendance != null) {
            VehicleScreen(garage, attendance)
        } else if (tab == 2 && orders.open != null) {
            OrderScreen(orders)
        } else {
            val title = when (tab) {
                0 -> R.string.garage_title
                1 -> R.string.valeta_title
                else -> R.string.orders_title
            }
            AppHeader(stringResource(title), userName, themeLabel, onTheme, onSignOut)
            Box(modifier = Modifier.weight(1f)) {
                when (tab) {
                    0 -> GarageTab(garage)
                    1 -> ValetaTab(valeta)
                    else -> OrdersTab(orders)
                }
            }
            BottomNav(
                listOf(R.string.tab_garage to "🔧", R.string.tab_valeta to "🔍", R.string.tab_maintenance to "🛠"),
                tab,
            ) { tab = it }
        }
    }
    if (garage.picking) VehiclePicker(garage)
}

@Composable
private fun GarageTab(state: MechanicState) {
    Column(modifier = Modifier.fillMaxSize()) {
        GroupChips(state.group) { state.group = it }
        state.error?.let { ErrorBanner(it) }
        QueueList(state)
    }
}

@Composable
private fun GroupChips(selected: String, onSelect: (String) -> Unit) {
    val options = listOf(
        GarageRules.ALL to R.string.garage_filter_all,
        GarageRules.EXTERNAL to R.string.garage_filter_external,
        GarageRules.INTERNAL to R.string.garage_filter_internal,
    )
    Row(modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
        options.forEach { (group, label) ->
            FilterChip(
                selected = selected == group,
                onClick = { onSelect(group) },
                label = { Text(stringResource(label), fontSize = 15.sp) },
                colors = FilterChipDefaults.filterChipColors(
                    selectedContainerColor = token(R.attr.hermesAccent),
                    selectedLabelColor = Color.White,
                    labelColor = token(R.attr.hermesTextPrimary),
                ),
            )
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun QueueList(state: MechanicState) {
    val queue = state.queue
    PullToRefreshBox(isRefreshing = state.refreshing, onRefresh = { state.refresh() }, modifier = Modifier.fillMaxSize()) {
        Box(modifier = Modifier.fillMaxSize()) {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                verticalArrangement = Arrangement.spacedBy(10.dp),
                contentPadding = androidx.compose.foundation.layout.PaddingValues(start = 16.dp, end = 16.dp, top = 4.dp, bottom = 96.dp),
            ) {
                if (queue != null && queue.isEmpty()) {
                    item { Text(stringResource(R.string.garage_empty), color = token(R.attr.hermesTextSecondary), modifier = Modifier.padding(24.dp)) }
                }
                items(queue.orEmpty(), key = { it.vehicleUuid }) { entry -> QueueCard(entry) { state.show(entry) } }
            }
            Button(
                onClick = { state.startPicking() },
                modifier = Modifier.align(Alignment.BottomCenter).padding(16.dp).fillMaxWidth().height(56.dp),
                shape = RoundedCornerShape(16.dp),
                colors = ButtonDefaults.buttonColors(containerColor = token(R.attr.hermesAccent), contentColor = Color.White),
            ) { Text(stringResource(R.string.garage_open_triage), fontSize = 17.sp, fontWeight = FontWeight.SemiBold) }
        }
    }
}

@Composable
private fun QueueCard(entry: GarageQueueEntry, onClick: () -> Unit) {
    val ok = token(R.attr.hermesOk)
    val warning = token(R.attr.hermesWarning)
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(token(R.attr.hermesSurface), RoundedCornerShape(16.dp))
            .border(BorderStroke(1.dp, token(R.attr.hermesOutline)), RoundedCornerShape(16.dp))
            .clickable(onClick = onClick)
            .padding(16.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(entry.prefix, color = token(R.attr.hermesTextPrimary), fontSize = 26.sp, fontWeight = FontWeight.Bold)
            if (entry.calledByManager) Pill(stringResource(R.string.garage_called), warning)
            if (entry.fillTankAlert) Pill(stringResource(R.string.garage_tank_low), warning)
            Spacer(Modifier.weight(1f))
            entry.nextDeparture?.let {
                Text(stringResource(R.string.garage_departure, Instants.toLocal(it).takeLast(5)), color = token(R.attr.hermesTextSecondary), fontSize = 13.sp)
            }
        }
        Spacer(Modifier.height(6.dp))
        if (entry.requiredPending.isEmpty()) {
            Text(stringResource(R.string.garage_all_done), color = ok, fontSize = 15.sp, fontWeight = FontWeight.SemiBold)
        } else {
            Text(
                stringResource(R.string.garage_pending_prefix, entry.requiredPending.joinToString(" · ")),
                color = token(R.attr.hermesTextSecondary),
                fontSize = 15.sp,
            )
        }
    }
}

@Composable
internal fun VehicleScreen(state: MechanicState, attendance: GarageAttendance) {
    val services = GarageRules.ordered(GarageRules.visible(attendance.services, state.group))
    Row(
        modifier = Modifier.fillMaxWidth().padding(start = 8.dp, end = 16.dp, top = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        TextButton(onClick = { state.close() }) { Text("‹ ${stringResource(R.string.garage_back)}", color = token(R.attr.hermesAccent), fontSize = 16.sp) }
        Spacer(Modifier.weight(1f))
        Text(state.openPrefix, color = token(R.attr.hermesTextPrimary), fontSize = 30.sp, fontWeight = FontWeight.Bold)
    }
    GroupChips(state.group) { state.group = it }
    val attended = GarageRules.attended(services)
    Column(modifier = Modifier.padding(horizontal = 20.dp)) {
        Text(stringResource(R.string.garage_progress, attended, services.size), color = token(R.attr.hermesTextSecondary), fontSize = 14.sp)
        Spacer(Modifier.height(6.dp))
        LinearProgressIndicator(
            progress = { if (services.isEmpty()) 0f else attended / services.size.toFloat() },
            modifier = Modifier.fillMaxWidth().height(6.dp),
            color = token(R.attr.hermesOk),
            trackColor = token(R.attr.hermesOutline),
        )
    }
    state.error?.let { ErrorBanner(it) }
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.spacedBy(10.dp),
        contentPadding = androidx.compose.foundation.layout.PaddingValues(16.dp),
    ) {
        if (services.isEmpty()) item { Text(stringResource(R.string.garage_no_services), color = token(R.attr.hermesTextSecondary)) }
        items(services, key = { it.uuid ?: it.name }) { service ->
            ServiceCard(service, busy = state.saving == service.uuid) { state.mark(service, it) }
        }
    }
}

@Composable
private fun ServiceCard(service: GarageService, busy: Boolean, onMark: (String) -> Unit) {
    val current = service.effectiveState
    val color = when (current) {
        STATE_PERFORMED -> token(R.attr.hermesOk)
        STATE_NOT_DONE -> token(R.attr.hermesError)
        STATE_NOT_NEEDED -> token(R.attr.hermesTextSecondary)
        else -> token(R.attr.hermesWarning)
    }
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(token(R.attr.hermesSurface), RoundedCornerShape(16.dp))
            .border(BorderStroke(1.dp, if (current == STATE_PENDING) color.copy(alpha = 0.6f) else token(R.attr.hermesOutline)), RoundedCornerShape(16.dp))
            .padding(16.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(service.name, color = token(R.attr.hermesTextPrimary), fontSize = 18.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
            if (service.requiredForDeparture) Pill(stringResource(R.string.garage_required), token(R.attr.hermesWarning))
            Pill(stateLabel(current), color)
        }
        Spacer(Modifier.height(12.dp))
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            listOf(STATE_PERFORMED, STATE_NOT_NEEDED, STATE_NOT_DONE).forEach { target ->
                val selected = current == target
                val label = stateLabel(target)
                val modifier = Modifier.weight(1f).height(48.dp)
                if (selected) {
                    Button(
                        onClick = { onMark(target) }, enabled = !busy, modifier = modifier, shape = RoundedCornerShape(12.dp),
                        colors = ButtonDefaults.buttonColors(containerColor = color, contentColor = Color.White),
                    ) { Text(label, fontSize = 14.sp, maxLines = 1) }
                } else {
                    OutlinedButton(
                        onClick = { onMark(target) }, enabled = !busy, modifier = modifier, shape = RoundedCornerShape(12.dp),
                        border = BorderStroke(1.dp, token(R.attr.hermesOutline)),
                    ) { Text(label, fontSize = 14.sp, maxLines = 1, color = token(R.attr.hermesTextPrimary)) }
                }
            }
        }
    }
}

@Composable
private fun stateLabel(state: String): String = stringResource(
    when (state) {
        STATE_PERFORMED -> R.string.garage_state_performed
        STATE_NOT_NEEDED -> R.string.garage_state_not_needed
        STATE_NOT_DONE -> R.string.garage_state_not_done
        else -> R.string.garage_state_pending
    },
)

@Composable
private fun VehiclePicker(state: MechanicState) {
    var query by remember { mutableStateOf("") }
    val inYard = state.queue.orEmpty().map { it.vehicleUuid }.toSet()
    val candidates = state.vehicles.orEmpty()
        .filter { it.uuid !in inYard }
        .filter { query.isBlank() || (it.prefix ?: "").contains(query.trim(), ignoreCase = true) || it.plate.contains(query.trim(), ignoreCase = true) }
    AlertDialog(
        onDismissRequest = { state.picking = false },
        title = { Text(stringResource(R.string.garage_open_triage_title)) },
        text = {
            Column {
                OutlinedTextField(
                    value = query, onValueChange = { query = it }, singleLine = true,
                    placeholder = { Text(stringResource(R.string.garage_search_hint)) }, modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(8.dp))
                LazyColumn(modifier = Modifier.height(320.dp)) {
                    if (candidates.isEmpty()) item { Text(stringResource(R.string.garage_no_vehicles), color = MaterialTheme.colorScheme.onSurfaceVariant) }
                    items(candidates, key = { it.uuid }) { vehicle ->
                        Row(
                            modifier = Modifier.fillMaxWidth().clickable { state.openTriage(vehicle) }.padding(vertical = 14.dp),
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                        ) {
                            Text(vehicle.prefix ?: vehicle.plate, fontSize = 20.sp, fontWeight = FontWeight.Bold)
                            Text(vehicle.plate, fontSize = 16.sp, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        }
                    }
                }
            }
        },
        confirmButton = {},
        dismissButton = { TextButton(onClick = { state.picking = false }) { Text(stringResource(R.string.cancel)) } },
    )
}
