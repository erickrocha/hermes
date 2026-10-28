package com.erocha.hermes.client.driver

import android.content.ActivityNotFoundException
import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Checkbox
import androidx.compose.material3.CheckboxDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.erocha.hermes.client.shared.ANSWER_CONFORMING
import com.erocha.hermes.client.shared.ANSWER_NON_CONFORMING
import com.erocha.hermes.client.shared.AppAccess
import com.erocha.hermes.client.shared.ExtraTrip
import com.erocha.hermes.client.shared.FuelEntry
import com.erocha.hermes.client.shared.FuelReceiptParser
import com.erocha.hermes.client.shared.Vehicle
import com.erocha.hermes.client.shared.VehicleAction
import kotlinx.coroutines.launch
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

@Composable
internal fun DriverApp(host: FleetHost, userName: String, profileLabel: String, themeLabel: String, onTheme: () -> Unit, onSignOut: () -> Unit) {
    HermesMaterialTheme { DriverBody(host, userName, profileLabel, themeLabel, onTheme, onSignOut) }
}

/** The driver's tabs; trips and fuellings appear only for whoever may read them (`C-035`). */
private enum class DriverTab(val label: Int, val icon: String) {
    HOME(R.string.tab_home, "🚌"),
    TRIPS(R.string.tab_trips, "🗺"),
    FUEL(R.string.tab_fuel, "⛽"),
}

private fun driverTabs(access: AppAccess): List<DriverTab> = buildList {
    add(DriverTab.HOME)
    if (access.trips) add(DriverTab.TRIPS)
    if (access.fuelEntries) add(DriverTab.FUEL)
}

@Composable
private fun DriverBody(host: FleetHost, userName: String, profileLabel: String, themeLabel: String, onTheme: () -> Unit, onSignOut: () -> Unit) {
    val state = remember { DriverState(host) }
    val tabs = remember(host.access) { driverTabs(host.access) }
    var tabIndex by rememberSaveable { mutableIntStateOf(0) }
    val tab = tabs[tabIndex.coerceIn(0, tabs.lastIndex)]
    LaunchedEffect(Unit) {
        state.loadHome()
        if (DriverTab.TRIPS in tabs) state.loadTrips()
        if (DriverTab.FUEL in tabs) state.loadEntries()
    }
    BackHandler(enabled = state.screen !is DriverScreen.Tabs) { state.screen = DriverScreen.Tabs }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(token(R.attr.hermesScreenBackground))
            .statusBarsPadding()
            .navigationBarsPadding(),
    ) {
        when (val screen = state.screen) {
            is DriverScreen.Checklist -> ChecklistScreen(state, screen.form)
            is DriverScreen.Position -> PositionScreen(state, host, screen.vehicle)
            is DriverScreen.Receipt -> ReceiptScreen(state, screen.form)
            DriverScreen.Tabs -> {
                val title = when (tab) {
                    DriverTab.HOME -> stringResource(R.string.authenticated_title, userName.substringBefore(' '))
                    DriverTab.TRIPS -> stringResource(R.string.trips_title)
                    DriverTab.FUEL -> stringResource(R.string.my_fuel_entries)
                }
                AppHeader(title, profileLabel, themeLabel, onTheme, onSignOut)
                Box(modifier = Modifier.weight(1f)) {
                    when (tab) {
                        DriverTab.HOME -> HomeTab(state)
                        DriverTab.TRIPS -> TripsTab(state)
                        DriverTab.FUEL -> FuelTab(state)
                    }
                }
                if (tabs.size > 1) BottomNav(tabs.map { it.label to it.icon }, tabs.indexOf(tab)) { tabIndex = it }
            }
        }
    }
}

@Composable
private fun ScreenTop(title: String, onBack: () -> Unit) {
    Row(modifier = Modifier.fillMaxWidth().padding(start = 8.dp, end = 20.dp, top = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        TextButton(onClick = onBack) { Text("‹ ${stringResource(R.string.back_button)}", color = token(R.attr.hermesAccent), fontSize = 16.sp) }
        Spacer(Modifier.weight(1f))
        Text(title, color = token(R.attr.hermesTextPrimary), fontSize = 20.sp, fontWeight = FontWeight.Bold)
    }
}

@Composable
private fun Card(modifier: Modifier = Modifier, content: @Composable ColumnScope.() -> Unit) {
    Column(
        modifier = modifier
            .fillMaxWidth()
            .background(token(R.attr.hermesSurface), RoundedCornerShape(20.dp))
            .border(BorderStroke(1.dp, token(R.attr.hermesOutline)), RoundedCornerShape(20.dp))
            .padding(18.dp),
        content = content,
    )
}

@Composable
private fun PrimaryButton(label: String, enabled: Boolean = true, onClick: () -> Unit) {
    Button(
        onClick = onClick, enabled = enabled, modifier = Modifier.fillMaxWidth().height(56.dp), shape = RoundedCornerShape(16.dp),
        colors = ButtonDefaults.buttonColors(containerColor = token(R.attr.hermesAccent), contentColor = Color.White),
    ) { Text(label, fontSize = 17.sp, fontWeight = FontWeight.SemiBold) }
}

@Composable
private fun SecondaryButton(label: String, modifier: Modifier = Modifier, enabled: Boolean = true, onClick: () -> Unit) {
    OutlinedButton(
        onClick = onClick, enabled = enabled, modifier = modifier.height(52.dp), shape = RoundedCornerShape(14.dp),
        border = BorderStroke(1.dp, token(R.attr.hermesAccent)),
    ) { Text(label, color = token(R.attr.hermesAccent), fontSize = 15.sp, maxLines = 1) }
}

// ---- Home: the assigned vehicle(s) ----

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun HomeTab(state: DriverState) {
    PullToRefreshBox(isRefreshing = state.refreshing, onRefresh = { state.loadHome() }, modifier = Modifier.fillMaxSize()) {
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            verticalArrangement = Arrangement.spacedBy(14.dp),
            contentPadding = PaddingValues(16.dp),
        ) {
            state.homeMessage?.let { item { Text(it, color = token(R.attr.hermesOk), fontWeight = FontWeight.SemiBold) } }
            state.error?.let { item { Text(it, color = token(R.attr.hermesError)) } }
            val rows = state.rows
            if (rows != null && rows.isEmpty()) item { Text(stringResource(R.string.no_vehicle), color = token(R.attr.hermesTextSecondary)) }
            items(rows.orEmpty(), key = { it.vehicle.uuid }) { row -> VehicleCard(state, row) }
        }
    }
}

@Composable
private fun VehicleCard(state: DriverState, row: VehicleRow) {
    val vehicle = row.vehicle
    Card {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(vehicle.prefix ?: vehicle.plate, color = token(R.attr.hermesTextPrimary), fontSize = 34.sp, fontWeight = FontWeight.Bold)
            Spacer(Modifier.weight(1f))
            Pill(statusLabel(vehicle.status), if (vehicle.status == "Active") token(R.attr.hermesOk) else token(R.attr.hermesWarning))
        }
        Text(stringResource(R.string.vehicle_line, vehicle.plate, vehicle.model), color = token(R.attr.hermesTextSecondary), fontSize = 15.sp)
        vehicle.odometerKm?.let {
            Text(stringResource(R.string.vehicle_odometer, kmText(it)), color = token(R.attr.hermesTextSecondary), fontSize = 14.sp, modifier = Modifier.padding(top = 2.dp))
        }
        Spacer(Modifier.height(16.dp))
        val access = state.access
        when (row.action) {
            VehicleAction.Depart -> if (access.checklist) PrimaryButton(stringResource(R.string.register_departure), enabled = !state.busy) { state.openChecklist(row) }
            is VehicleAction.Return -> if (access.checklist) PrimaryButton(stringResource(R.string.register_return), enabled = !state.busy) { state.openChecklist(row) }
            VehicleAction.HeldByAnother -> Text(stringResource(R.string.held_by_another), color = token(R.attr.hermesError), fontSize = 15.sp)
        }
        if (access.position || access.fuelReceipt) {
            Spacer(Modifier.height(10.dp))
            Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                if (access.position) SecondaryButton(stringResource(R.string.view_position), Modifier.weight(1f)) { state.openPosition(vehicle) }
                if (access.fuelReceipt) SecondaryButton(stringResource(R.string.fuel_new), Modifier.weight(1f)) { state.openReceipt(vehicle) }
            }
        }
    }
}

@Composable
private fun statusLabel(status: String): String = stringResource(
    when (status) {
        "Active" -> R.string.status_Active
        "Maintenance" -> R.string.status_Maintenance
        "Transit" -> R.string.status_Transit
        "Reserved" -> R.string.status_Reserved
        "Inactive" -> R.string.status_Inactive
        else -> return status
    },
)

private fun kmText(km: Double) =
    if (km % 1.0 == 0.0) String.format(Locale("pt", "BR"), "%,d", km.toLong()) else String.format(Locale("pt", "BR"), "%,.1f", km)

// ---- Trips ----

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun TripsTab(state: DriverState) {
    val today = remember { SimpleDateFormat("yyyy-MM-dd", Locale.US).format(Date()) }
    val trips = state.trips
    val (upcoming, past) = trips.orEmpty().partition { it.tripDate >= today && it.status != "Cancelled" }
    PullToRefreshBox(isRefreshing = state.refreshing, onRefresh = { state.loadTrips() }, modifier = Modifier.fillMaxSize()) {
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            verticalArrangement = Arrangement.spacedBy(12.dp),
            contentPadding = PaddingValues(16.dp),
        ) {
            if (trips != null && trips.isEmpty()) item { Text(stringResource(R.string.no_trips), color = token(R.attr.hermesTextSecondary)) }
            if (upcoming.isNotEmpty()) item { SectionTitle(stringResource(R.string.trips_upcoming)) }
            items(upcoming.sortedBy { it.tripDate + it.startTime.orEmpty() }, key = { it.uuid }) { TripCard(it, highlight = true) }
            if (past.isNotEmpty()) item { SectionTitle(stringResource(R.string.trips_past)) }
            items(past, key = { it.uuid }) { TripCard(it, highlight = false) }
        }
    }
}

@Composable
private fun SectionTitle(text: String) {
    Text(text, color = token(R.attr.hermesTextSecondary), fontSize = 13.sp, fontWeight = FontWeight.Bold, modifier = Modifier.padding(top = 6.dp, start = 4.dp))
}

@Composable
private fun TripCard(trip: ExtraTrip, highlight: Boolean) {
    val accent = if (highlight) token(R.attr.hermesAccent) else token(R.attr.hermesTextSecondary)
    val day = trip.tripDate.takeLast(2)
    val month = remember(trip.tripDate) {
        runCatching { SimpleDateFormat("MMM", Locale("pt", "BR")).format(SimpleDateFormat("yyyy-MM-dd", Locale.US).parse(trip.tripDate)!!) }.getOrDefault("")
            .uppercase(Locale("pt", "BR")).trimEnd('.')
    }
    Card {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(16.dp)) {
            Column(
                modifier = Modifier.background(accent.copy(alpha = 0.12f), RoundedCornerShape(14.dp)).padding(horizontal = 16.dp, vertical = 10.dp),
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                Text(day, color = accent, fontSize = 28.sp, fontWeight = FontWeight.Bold)
                Text(month, color = accent, fontSize = 12.sp, fontWeight = FontWeight.Bold)
            }
            Column(modifier = Modifier.weight(1f)) {
                Text(Instants.time(trip.startTime).ifEmpty { "--:--" }, color = token(R.attr.hermesTextPrimary), fontSize = 22.sp, fontWeight = FontWeight.Bold)
                if (!trip.originCity.isNullOrBlank() || !trip.destination.isNullOrBlank()) {
                    Text(
                        stringResource(R.string.trip_route, trip.originCity.orEmpty(), trip.destination.orEmpty()),
                        color = token(R.attr.hermesTextPrimary), fontSize = 16.sp,
                        maxLines = 2, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
                    )
                }
                trip.stops?.trim('"', ' ')?.takeIf { it.isNotBlank() }?.let {
                    Text(
                        stringResource(R.string.trip_stops, it), color = token(R.attr.hermesTextSecondary), fontSize = 13.sp,
                        maxLines = 2, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis,
                    )
                }
            }
        }
        Spacer(Modifier.height(10.dp))
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Pill(tripStatus(trip.status), tripStatusColor(trip.status))
            Text(trip.orderCode, color = token(R.attr.hermesTextSecondary), fontSize = 13.sp)
            trip.returnDate?.takeIf { it.isNotBlank() }?.let {
                Spacer(Modifier.weight(1f))
                Text("↩ ${Instants.date(it)} ${Instants.time(trip.returnTime)}".trim(), color = token(R.attr.hermesTextSecondary), fontSize = 13.sp)
            }
        }
    }
}

@Composable
private fun tripStatus(status: String): String = stringResource(
    when (status) {
        "Scheduled" -> R.string.trip_status_Scheduled
        "Conflict" -> R.string.trip_status_Conflict
        "PendingSchedule" -> R.string.trip_status_PendingSchedule
        "Cancelled" -> R.string.trip_status_Cancelled
        else -> return status
    },
)

@Composable
private fun tripStatusColor(status: String): Color = when (status) {
    "Scheduled" -> token(R.attr.hermesOk)
    "Conflict" -> token(R.attr.hermesError)
    "Cancelled" -> token(R.attr.hermesTextSecondary)
    else -> token(R.attr.hermesWarning)
}

// ---- Fuel ----

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun FuelTab(state: DriverState) {
    Column(modifier = Modifier.fillMaxSize()) {
        val hasVehicle = state.rows.orEmpty().isNotEmpty()
        if (state.access.fuelReceipt) {
            Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
                PrimaryButton(stringResource(R.string.fuel_new), enabled = hasVehicle) { state.openReceipt() }
                if (state.rows != null && !hasVehicle) Text(stringResource(R.string.no_vehicle), color = token(R.attr.hermesTextSecondary), fontSize = 13.sp)
            }
        }
        PullToRefreshBox(isRefreshing = state.refreshing, onRefresh = { state.loadEntries() }, modifier = Modifier.fillMaxSize()) {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                verticalArrangement = Arrangement.spacedBy(10.dp),
                contentPadding = PaddingValues(start = 16.dp, end = 16.dp, bottom = 16.dp),
            ) {
                state.homeMessage?.let { item { Text(it, color = token(R.attr.hermesOk), fontWeight = FontWeight.SemiBold) } }
                val entries = state.entries
                if (entries != null && entries.isEmpty()) item { Text(stringResource(R.string.no_fuel_entries), color = token(R.attr.hermesTextSecondary)) }
                items(entries.orEmpty(), key = { it.uuid ?: it.recordedAt }) { FuelCard(it) }
            }
        }
    }
}

@Composable
private fun FuelCard(entry: FuelEntry) {
    Card {
        Row(verticalAlignment = Alignment.CenterVertically) {
            Column(modifier = Modifier.weight(1f)) {
                Text(Instants.toLocal(entry.recordedAt), color = token(R.attr.hermesTextSecondary), fontSize = 13.sp)
                Text(
                    String.format(Locale("pt", "BR"), "%.1f L", entry.volumeLiters),
                    color = token(R.attr.hermesTextPrimary), fontSize = 24.sp, fontWeight = FontWeight.Bold,
                )
                entry.station?.takeIf { it.isNotBlank() }?.let { Text(it, color = token(R.attr.hermesTextSecondary), fontSize = 14.sp) }
            }
            Text("R$ ${Instants.money(entry.valueCents)}", color = token(R.attr.hermesAccent), fontSize = 20.sp, fontWeight = FontWeight.Bold)
        }
    }
}

// ---- Checklist ----

@Composable
private fun ColumnScope.ChecklistScreen(state: DriverState, form: ChecklistForm) {
    val vehicle = form.row.vehicle
    ScreenTop(stringResource(if (form.isReturn) R.string.return_form_title else R.string.departure_form_title)) { state.screen = DriverScreen.Tabs }
    Text(
        "${vehicle.prefix ?: vehicle.plate} · ${vehicle.plate}",
        color = token(R.attr.hermesTextSecondary), fontSize = 15.sp, modifier = Modifier.padding(horizontal = 20.dp),
    )
    LazyColumn(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(10.dp), contentPadding = PaddingValues(16.dp)) {
        item {
            OutlinedTextField(
                value = form.odometer, onValueChange = { form.odometer = it }, singleLine = true,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                label = { Text(stringResource(R.string.odometer_hint)) }, modifier = Modifier.fillMaxWidth(),
            )
        }
        items(form.template.items, key = { it.uuid }) { item ->
            val answer = form.answers[item.uuid]
            Card {
                Text(item.description, color = token(R.attr.hermesTextPrimary), fontSize = 17.sp, fontWeight = FontWeight.SemiBold)
                Spacer(Modifier.height(10.dp))
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    ChoiceButton(stringResource(R.string.answer_conforming), answer == ANSWER_CONFORMING, token(R.attr.hermesOk), Modifier.weight(1f)) { form.answers[item.uuid] = ANSWER_CONFORMING }
                    ChoiceButton(stringResource(R.string.answer_non_conforming), answer == ANSWER_NON_CONFORMING, token(R.attr.hermesError), Modifier.weight(1f)) { form.answers[item.uuid] = ANSWER_NON_CONFORMING }
                }
                if (answer == ANSWER_NON_CONFORMING) {
                    Spacer(Modifier.height(8.dp))
                    OutlinedTextField(
                        value = form.observations[item.uuid].orEmpty(), onValueChange = { form.observations[item.uuid] = it },
                        label = { Text(stringResource(R.string.observation_hint)) }, modifier = Modifier.fillMaxWidth(),
                    )
                }
            }
        }
        item {
            OutlinedTextField(
                value = form.notes, onValueChange = { form.notes = it },
                label = { Text(stringResource(R.string.notes_hint)) }, modifier = Modifier.fillMaxWidth(),
            )
        }
    }
    Column(modifier = Modifier.padding(16.dp)) {
        form.message?.let { Text(it, color = token(R.attr.hermesError), fontSize = 14.sp, modifier = Modifier.padding(bottom = 8.dp)) }
        PrimaryButton(
            stringResource(if (form.submitting) R.string.submitting else if (form.needsCheck) R.string.check_button else R.string.submit_button),
            enabled = form.canSubmit || (form.needsCheck && !form.submitting),
        ) { if (form.needsCheck) state.check(form) else state.submit(form) }
    }
}

// ---- Position ----

@Composable
private fun ColumnScope.PositionScreen(state: DriverState, host: FleetHost, vehicle: Vehicle) {
    ScreenTop(stringResource(R.string.position_title)) { state.screen = DriverScreen.Tabs }
    val tracking = state.tracking
    val point = tracking?.position
    Column(modifier = Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Card {
            Text(vehicle.prefix ?: vehicle.plate, color = token(R.attr.hermesTextPrimary), fontSize = 30.sp, fontWeight = FontWeight.Bold)
            Text(stringResource(R.string.vehicle_line, vehicle.plate, vehicle.model), color = token(R.attr.hermesTextSecondary), fontSize = 15.sp)
            Spacer(Modifier.height(12.dp))
            when {
                state.refreshing && tracking == null -> Text(stringResource(R.string.loading), color = token(R.attr.hermesTextSecondary))
                state.trackingError != null -> Text(state.trackingError.orEmpty(), color = token(R.attr.hermesError))
                tracking != null && point == null -> Text(stringResource(R.string.position_none), color = token(R.attr.hermesTextSecondary))
                tracking != null && point != null -> {
                    // A reading that is not live is never passed off as live (HRMS-929).
                    Pill(stringResource(if (tracking.positionLive) R.string.position_live_pill else R.string.position_stale_pill), token(if (tracking.positionLive) R.attr.hermesOk else R.attr.hermesWarning))
                    Spacer(Modifier.height(8.dp))
                    Text(stringResource(R.string.position_coordinates, point.latitude.toString(), point.longitude.toString()), color = token(R.attr.hermesTextPrimary), fontSize = 16.sp)
                    Text(stringResource(R.string.position_updated, Instants.toLocal(tracking.positionReportedAt ?: tracking.reportedAt)), color = token(R.attr.hermesTextSecondary), fontSize = 14.sp)
                }
            }
            tracking?.let {
                Spacer(Modifier.height(8.dp))
                Text(stringResource(R.string.ignition_line, ignitionLabel(it.ignition)), color = token(R.attr.hermesTextPrimary), fontSize = 15.sp)
                if (!it.trustworthy) Text(stringResource(R.string.ignition_untrusted), color = token(R.attr.hermesError), fontSize = 13.sp)
            }
        }
    }
    var noMap by remember { mutableIntStateOf(0) }
    Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        if (noMap > 0) Text(stringResource(R.string.no_map_app), color = token(R.attr.hermesError), fontSize = 14.sp)
        point?.let { SecondaryButton(stringResource(R.string.open_map), Modifier.fillMaxWidth()) { if (!host.openMap(it.latitude, it.longitude)) noMap++ } }
        PrimaryButton(stringResource(R.string.refresh_position), enabled = !state.refreshing) { state.refreshPosition(vehicle) }
    }
}

@Composable
private fun ignitionLabel(state: String): String = stringResource(
    when (state) {
        "On" -> R.string.ignition_On
        "Off" -> R.string.ignition_Off
        else -> R.string.ignition_Unknown
    },
)

// ---- Fuel receipt, with the scanner ----

@Composable
private fun ColumnScope.ReceiptScreen(state: DriverState, form: ReceiptForm) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var pending by remember { androidx.compose.runtime.mutableStateOf<Pair<File, android.net.Uri>?>(null) }
    var noCamera by remember { mutableIntStateOf(0) }
    val launcher = rememberLauncherForActivityResult(ActivityResultContracts.TakePicture()) { taken ->
        val photo = pending
        pending = null
        if (photo == null) return@rememberLauncherForActivityResult
        if (!taken) { photo.first.delete(); return@rememberLauncherForActivityResult }
        form.scanning = true
        scope.launch {
            val text = runCatching { ReceiptScanner.read(context, photo.first, photo.second) }.getOrNull()
            form.scanning = false
            if (text == null) form.scanFailed = true else form.apply(FuelReceiptParser.parse(text))
        }
    }
    val vehicles = state.rows.orEmpty().map { it.vehicle }

    ScreenTop(stringResource(R.string.fuel_form_title)) { state.screen = DriverScreen.Tabs }
    Column(
        modifier = Modifier.weight(1f).verticalScroll(rememberScrollState()).padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        if (vehicles.size > 1) {
            Text(stringResource(R.string.fuel_vehicle_pick), color = token(R.attr.hermesTextSecondary), fontSize = 13.sp)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                vehicles.forEach { vehicle ->
                    FilterChip(selected = form.vehicle.uuid == vehicle.uuid, onClick = { form.choose(vehicle) }, label = { Text(vehicle.prefix ?: vehicle.plate) })
                }
            }
        } else {
            Text("${form.vehicle.prefix ?: form.vehicle.plate} · ${form.vehicle.plate}", color = token(R.attr.hermesTextSecondary), fontSize = 15.sp)
        }

        Card {
            Text(stringResource(R.string.scan_receipt_hint), color = token(R.attr.hermesTextSecondary), fontSize = 14.sp)
            Spacer(Modifier.height(10.dp))
            PrimaryButton(
                stringResource(when { form.scanning -> R.string.scan_reading; form.scanned != null -> R.string.scan_again; else -> R.string.scan_receipt }),
                enabled = !form.scanning && !form.submitting,
            ) {
                val photo = ReceiptScanner.newPhoto(context)
                pending = photo
                try { launcher.launch(photo.second) } catch (_: ActivityNotFoundException) { pending = null; photo.first.delete(); noCamera++ }
            }
            if (noCamera > 0) Text(stringResource(R.string.scan_no_camera), color = token(R.attr.hermesError), fontSize = 14.sp, modifier = Modifier.padding(top = 8.dp))
            form.scanned?.let {
                Text(
                    stringResource(if (form.scanFailed) R.string.scan_failed else R.string.scan_result),
                    color = token(if (form.scanFailed) R.attr.hermesWarning else R.attr.hermesOk),
                    fontSize = 14.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(top = 8.dp),
                )
                if (!form.scanFailed && form.odometer.isBlank() && state.access.odometer) {
                    Text(stringResource(R.string.scan_odometer_note), color = token(R.attr.hermesTextSecondary), fontSize = 13.sp)
                }
            }
        }

        OutlinedTextField(value = form.prefix, onValueChange = { form.prefix = it }, singleLine = true, label = { Text(stringResource(R.string.prefix_hint)) }, modifier = Modifier.fillMaxWidth())
        Row(horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            OutlinedTextField(
                value = form.liters, onValueChange = { form.liters = it }, singleLine = true, modifier = Modifier.weight(1f),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal), label = { Text(stringResource(R.string.liters_hint)) },
            )
            OutlinedTextField(
                value = form.value, onValueChange = { form.value = it }, singleLine = true, modifier = Modifier.weight(1f),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal), label = { Text(stringResource(R.string.value_hint)) },
            )
        }
        // The reading is optional on a receipt; only whoever may record the odometer is asked for it.
        if (state.access.odometer) {
            OutlinedTextField(
                value = form.odometer, onValueChange = { form.odometer = it }, singleLine = true, modifier = Modifier.fillMaxWidth(),
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal), label = { Text(stringResource(R.string.fuel_odometer_hint)) },
            )
        }
        OutlinedTextField(value = form.station, onValueChange = { form.station = it }, singleLine = true, label = { Text(stringResource(R.string.station_hint)) }, modifier = Modifier.fillMaxWidth())
        Row(verticalAlignment = Alignment.CenterVertically) {
            Checkbox(checked = form.fullTank, onCheckedChange = { form.fullTank = it }, colors = CheckboxDefaults.colors(checkedColor = token(R.attr.hermesAccent)))
            Text(stringResource(R.string.full_tank), color = token(R.attr.hermesTextPrimary), fontSize = 16.sp)
        }
    }
    Column(modifier = Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        form.message?.let { Text(it, color = token(R.attr.hermesError), fontSize = 14.sp) }
        PrimaryButton(stringResource(if (form.submitting) R.string.submitting else R.string.submit_button), enabled = form.canSubmit) { state.submitReceipt(form, override = false) }
        if (form.canOverride) SecondaryButton(stringResource(R.string.send_anyway), Modifier.fillMaxWidth(), enabled = form.canSubmit) { state.submitReceipt(form, override = true) }
    }
}
