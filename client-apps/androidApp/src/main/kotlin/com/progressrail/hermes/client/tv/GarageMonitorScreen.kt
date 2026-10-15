package com.progressrail.hermes.client.tv

import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.itemsIndexed
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.foundation.lazy.grid.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.colorResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.progressrail.hermes.client.shared.GarageCallListEntry
import com.progressrail.hermes.client.shared.GarageMatrix
import com.progressrail.hermes.client.shared.GarageMonitorCard
import com.progressrail.hermes.client.shared.GarageMonitorEntry
import java.text.DateFormat
import java.util.Date
import java.text.SimpleDateFormat
import java.util.Locale
import java.util.TimeZone

internal data class GarageMonitorUiState(
    val queue: List<GarageMonitorEntry> = emptyList(),
    val cards: List<GarageMonitorCard> = emptyList(),
    val cardsNotShown: Int = 0,
    val callList: List<GarageCallListEntry> = emptyList(),
    val matrix: GarageMatrix = GarageMatrix(),
    val lastUpdatedAtMillis: Long? = null,
    val isRefreshing: Boolean = true,
    val errorMessage: String? = null,
)

@Composable
internal fun GarageMonitorScreen(
    state: GarageMonitorUiState,
    onRefresh: () -> Unit,
    onRePair: () -> Unit,
) {
    val background = colorResource(R.color.monitor_background)
    val surface = colorResource(R.color.monitor_surface)
    val raisedSurface = colorResource(R.color.monitor_surface_raised)
    val textColor = colorResource(R.color.monitor_text)
    val mutedText = colorResource(R.color.monitor_text_muted)
    val accent = colorResource(R.color.monitor_accent)
    val warning = colorResource(R.color.monitor_warning)
    val danger = colorResource(R.color.monitor_danger)
    val outline = colorResource(R.color.monitor_outline)
    var selectedTab by rememberSaveable { mutableIntStateOf(0) }
    val firstTabRequester = remember { FocusRequester() }

    MaterialTheme(
        colorScheme = darkColorScheme(
            background = background,
            surface = surface,
            primary = accent,
            onPrimary = background,
            onBackground = textColor,
            onSurface = textColor,
            secondary = warning,
            error = danger,
        ),
    ) {
        LaunchedEffect(Unit) {
            firstTabRequester.requestFocus()
        }

        Column(
            modifier = Modifier
                .fillMaxSize()
                .background(background)
                .padding(horizontal = 48.dp, vertical = 30.dp),
        ) {
            Header(
                state = state,
                textColor = textColor,
                mutedText = mutedText,
                warning = warning,
                danger = danger,
                accent = accent,
            )
            Spacer(Modifier.height(22.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                val tabLabels = listOf(
                    stringResource(R.string.monitor_tab_queue),
                    stringResource(R.string.monitor_tab_fuel),
                    stringResource(R.string.monitor_tab_matrix),
                    stringResource(R.string.monitor_tab_calls),
                )
                tabLabels.forEachIndexed { index, label ->
                    DashboardTab(
                        label = label,
                        selected = selectedTab == index,
                        selectedColor = accent,
                        textColor = textColor,
                        outlineColor = outline,
                        modifier = Modifier.weight(1f),
                        focusRequester = if (index == 0) firstTabRequester else null,
                        onSelect = { selectedTab = index },
                    )
                }
                Spacer(Modifier.width(12.dp))
                DashboardAction(
                    label = stringResource(R.string.monitor_refresh),
                    enabled = !state.isRefreshing,
                    accent = accent,
                    textColor = textColor,
                    outlineColor = outline,
                    onClick = onRefresh,
                )
                DashboardAction(
                    label = stringResource(R.string.monitor_repair),
                    enabled = true,
                    accent = accent,
                    textColor = textColor,
                    outlineColor = outline,
                    onClick = onRePair,
                )
            }
            Spacer(Modifier.height(16.dp))
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(1.dp)
                    .background(outline),
            )
            Spacer(Modifier.height(18.dp))
            if (state.errorMessage != null) {
                Text(
                    text = state.errorMessage,
                    color = warning,
                    fontSize = 18.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Spacer(Modifier.height(12.dp))
            }
            Box(modifier = Modifier.fillMaxSize()) {
                when {
                    state.lastUpdatedAtMillis == null && state.isRefreshing ->
                        EmptyState(stringResource(R.string.monitor_loading), mutedText)
                    state.lastUpdatedAtMillis == null && state.errorMessage != null ->
                        EmptyState(state.errorMessage, warning)
                    else -> when (selectedTab) {
                        0 -> QueuePage(state.queue, textColor, mutedText, surface, raisedSurface, accent, warning, danger)
                        1 -> FuelPage(state.cards, state.cardsNotShown, textColor, mutedText, surface, raisedSurface, accent, warning)
                        2 -> MatrixPage(state.matrix, textColor, mutedText, surface, raisedSurface, accent)
                        else -> CallsPage(state.callList, textColor, mutedText, surface, raisedSurface, accent, warning)
                    }
                }
            }
        }
    }
}

@Composable
private fun Header(
    state: GarageMonitorUiState,
    textColor: Color,
    mutedText: Color,
    warning: Color,
    danger: Color,
    accent: Color,
) {
    val hasSnapshot = state.lastUpdatedAtMillis != null
    val status = when {
        state.errorMessage != null -> stringResource(R.string.monitor_stale)
        state.isRefreshing && !hasSnapshot -> stringResource(R.string.monitor_loading)
        state.isRefreshing -> stringResource(R.string.monitor_updating)
        else -> stringResource(R.string.monitor_online)
    }
    val statusColor = if (state.errorMessage != null) warning else if (state.isRefreshing) warning else accent

    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column {
            Text(
                text = stringResource(R.string.monitor_title),
                color = textColor,
                fontSize = 34.sp,
                fontWeight = FontWeight.Bold,
            )
            Text(
                text = stringResource(R.string.monitor_subtitle),
                color = mutedText,
                fontSize = 19.sp,
            )
        }
        Spacer(Modifier.weight(1f))
        Column(horizontalAlignment = Alignment.End) {
            Text(
                text = status,
                color = if (state.errorMessage != null) danger else statusColor,
                fontSize = 18.sp,
                fontWeight = FontWeight.SemiBold,
            )
            state.lastUpdatedAtMillis?.let { updatedAt ->
                val time = remember(updatedAt) {
                    DateFormat.getTimeInstance(DateFormat.SHORT, Locale("pt", "BR"))
                        .format(Date(updatedAt))
                }
                Text(
                    text = stringResource(R.string.monitor_last_updated, time),
                    color = mutedText,
                    fontSize = 16.sp,
                )
            }
        }
    }
}

@Composable
private fun DashboardTab(
    label: String,
    selected: Boolean,
    selectedColor: Color,
    textColor: Color,
    outlineColor: Color,
    modifier: Modifier = Modifier,
    focusRequester: FocusRequester? = null,
    onSelect: () -> Unit,
) {
    var focused by remember { androidx.compose.runtime.mutableStateOf(false) }
    val focusModifier = if (focusRequester == null) Modifier else Modifier.focusRequester(focusRequester)
    Box(
        modifier = modifier
            .then(focusModifier)
            .onFocusChanged {
                focused = it.isFocused
                if (it.isFocused) onSelect()
            }
            .focusable()
            .clickable(role = Role.Tab, onClick = onSelect)
            .height(54.dp)
            .background(if (focused || selected) selectedColor.copy(alpha = 0.18f) else Color.Transparent)
            .border(
                width = if (focused) 2.dp else 1.dp,
                color = if (focused || selected) selectedColor else outlineColor,
                shape = androidx.compose.foundation.shape.RoundedCornerShape(6.dp),
            )
            .padding(horizontal = 14.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            color = if (focused || selected) selectedColor else textColor,
            fontSize = 18.sp,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
    }
}

@Composable
private fun DashboardAction(
    label: String,
    enabled: Boolean,
    accent: Color,
    textColor: Color,
    outlineColor: Color,
    onClick: () -> Unit,
) {
    var focused by remember { androidx.compose.runtime.mutableStateOf(false) }
    Box(
        modifier = Modifier
            .onFocusChanged { focused = it.isFocused }
            .focusable(enabled = enabled)
            .clickable(enabled = enabled, onClick = onClick)
            .height(54.dp)
            .background(if (focused) accent.copy(alpha = 0.18f) else Color.Transparent)
            .border(
                width = if (focused) 2.dp else 1.dp,
                color = if (focused) accent else outlineColor,
                shape = androidx.compose.foundation.shape.RoundedCornerShape(6.dp),
            )
            .padding(horizontal = 16.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            color = if (enabled) textColor else textColor.copy(alpha = 0.45f),
            fontSize = 17.sp,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
        )
    }
}

@Composable
private fun QueuePage(
    entries: List<GarageMonitorEntry>,
    textColor: Color,
    mutedText: Color,
    surface: Color,
    raisedSurface: Color,
    accent: Color,
    warning: Color,
    danger: Color,
) {
    if (entries.isEmpty()) {
        EmptyState(stringResource(R.string.monitor_empty_queue), mutedText)
        return
    }
    Column(modifier = Modifier.fillMaxSize()) {
        Row(horizontalArrangement = Arrangement.spacedBy(28.dp)) {
            Text(stringResource(R.string.monitor_vehicle_count, entries.size), color = mutedText, fontSize = 18.sp)
            Text(
                stringResource(R.string.monitor_urgent_count, entries.count { it.urgent }),
                color = warning,
                fontSize = 18.sp,
                fontWeight = FontWeight.SemiBold,
            )
        }
        Spacer(Modifier.height(12.dp))
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            verticalArrangement = Arrangement.spacedBy(10.dp),
            contentPadding = PaddingValues(bottom = 12.dp),
        ) {
            itemsIndexed(entries, key = { index, entry -> "${entry.vehicleUuid}-$index" }) { _, entry ->
                QueueRow(entry, textColor, mutedText, surface, raisedSurface, accent, warning, danger)
            }
        }
    }
}

@Composable
private fun QueueRow(
    entry: GarageMonitorEntry,
    textColor: Color,
    mutedText: Color,
    surface: Color,
    raisedSurface: Color,
    accent: Color,
    warning: Color,
    danger: Color,
) {
    var focused by remember { androidx.compose.runtime.mutableStateOf(false) }
    val statusColor = when {
        entry.urgent -> danger
        entry.readiness.equals("Ready", ignoreCase = true) -> accent
        entry.readiness.equals("Alert", ignoreCase = true) -> warning
        else -> mutedText
    }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .onFocusChanged { focused = it.isFocused }
            .focusable()
            .background(if (focused) raisedSurface else surface, androidx.compose.foundation.shape.RoundedCornerShape(6.dp))
            .border(
                if (focused) 2.dp else 1.dp,
                if (focused) accent else Color.Transparent,
                androidx.compose.foundation.shape.RoundedCornerShape(6.dp),
            )
            .padding(horizontal = 20.dp, vertical = 13.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1.15f)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
                Text(entry.prefix, color = textColor, fontSize = 25.sp, fontWeight = FontWeight.Bold)
                if (entry.urgent) Text(stringResource(R.string.monitor_urgent), color = danger, fontSize = 15.sp, fontWeight = FontWeight.Bold)
                if (entry.calledByManager) Text(stringResource(R.string.monitor_called), color = warning, fontSize = 15.sp)
                if (entry.fillTankAlert) Text(stringResource(R.string.monitor_fill_tank), color = warning, fontSize = 15.sp)
            }
            Text(
                text = entry.requiredPending.takeIf { it.isNotEmpty() }?.joinToString(" | ")
                    ?: stringResource(R.string.monitor_unknown),
                color = mutedText,
                fontSize = 17.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Column(modifier = Modifier.weight(0.9f)) {
            Text(stringResource(R.string.monitor_readiness), color = mutedText, fontSize = 14.sp)
            Text(readinessLabel(entry.readiness), color = statusColor, fontSize = 19.sp, fontWeight = FontWeight.SemiBold)
        }
        Column(modifier = Modifier.weight(0.85f)) {
            Text(stringResource(R.string.monitor_departure), color = mutedText, fontSize = 14.sp)
            Text(formatTimestamp(entry.nextDeparture), color = textColor, fontSize = 17.sp, maxLines = 1)
        }
        Column(horizontalAlignment = Alignment.End) {
            Text(stringResource(R.string.monitor_priority), color = mutedText, fontSize = 14.sp)
            Text(entry.manualPriority?.toString() ?: "—", color = if (entry.manualPriority == null) mutedText else accent, fontSize = 20.sp, fontWeight = FontWeight.Bold)
        }
    }
}

@Composable
private fun FuelPage(
    cards: List<GarageMonitorCard>,
    notShown: Int,
    textColor: Color,
    mutedText: Color,
    surface: Color,
    raisedSurface: Color,
    accent: Color,
    warning: Color,
) {
    if (cards.isEmpty()) {
        EmptyState(stringResource(R.string.monitor_empty_cards), mutedText)
        return
    }
    Column(modifier = Modifier.fillMaxSize()) {
        LazyVerticalGrid(
            columns = GridCells.Adaptive(minSize = 310.dp),
            modifier = Modifier.weight(1f),
            verticalArrangement = Arrangement.spacedBy(14.dp),
            horizontalArrangement = Arrangement.spacedBy(14.dp),
            contentPadding = PaddingValues(bottom = 12.dp),
        ) {
            items(cards, key = { it.vehicleUuid }) { card ->
                FuelCard(card, textColor, mutedText, surface, raisedSurface, accent, warning)
            }
        }
        if (notShown > 0) {
            Text(stringResource(R.string.monitor_not_shown, notShown), color = mutedText, fontSize = 16.sp)
        }
    }
}

@Composable
private fun FuelCard(
    card: GarageMonitorCard,
    textColor: Color,
    mutedText: Color,
    surface: Color,
    raisedSurface: Color,
    accent: Color,
    warning: Color,
) {
    var focused by remember { androidx.compose.runtime.mutableStateOf(false) }
    Surface(
        color = if (focused) raisedSurface else surface,
        shape = androidx.compose.foundation.shape.RoundedCornerShape(6.dp),
        modifier = Modifier
            .fillMaxWidth()
            .onFocusChanged { focused = it.isFocused }
            .focusable()
            .border(
                if (focused) 2.dp else 1.dp,
                if (focused) accent else Color.Transparent,
                androidx.compose.foundation.shape.RoundedCornerShape(6.dp),
            ),
    ) {
        Column(modifier = Modifier.padding(horizontal = 20.dp, vertical = 16.dp)) {
            Text(card.prefix, color = textColor, fontSize = 27.sp, fontWeight = FontWeight.Bold)
            Spacer(Modifier.height(10.dp))
            Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.monitor_tank), color = mutedText, fontSize = 16.sp)
                Text(card.tankPercent?.let(::formatPercent) ?: stringResource(R.string.monitor_unknown), color = if (card.needsRefuel) warning else accent, fontSize = 25.sp, fontWeight = FontWeight.Bold)
                if (card.needsRefuel) Text(stringResource(R.string.monitor_needs_refuel), color = warning, fontSize = 16.sp)
            }
            Spacer(Modifier.height(8.dp))
            DataLine(stringResource(R.string.monitor_maintenance), card.maintenanceStatus, textColor, mutedText)
            DataLine(stringResource(R.string.monitor_last_cycle), card.lastGarageCycleDistanceKm?.let(::formatDistance), textColor, mutedText)
        }
    }
}

@Composable
private fun CallsPage(
    entries: List<GarageCallListEntry>,
    textColor: Color,
    mutedText: Color,
    surface: Color,
    raisedSurface: Color,
    accent: Color,
    warning: Color,
) {
    if (entries.isEmpty()) {
        EmptyState(stringResource(R.string.monitor_empty_calls), mutedText)
        return
    }
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.spacedBy(10.dp),
        contentPadding = PaddingValues(bottom = 12.dp),
    ) {
        itemsIndexed(entries, key = { index, entry -> "${entry.vehicleUuid}-$index" }) { _, entry ->
            CallRow(entry, textColor, mutedText, surface, raisedSurface, accent, warning)
        }
    }
}

@Composable
private fun CallRow(
    entry: GarageCallListEntry,
    textColor: Color,
    mutedText: Color,
    surface: Color,
    raisedSurface: Color,
    accent: Color,
    warning: Color,
) {
    var focused by remember { androidx.compose.runtime.mutableStateOf(false) }
    val reasons = buildList {
        if (entry.calledByManager) add(stringResource(R.string.monitor_reason_manager))
        if (entry.hasDepartureAhead) add(stringResource(R.string.monitor_reason_departure))
        if (entry.tankCalls) add(stringResource(R.string.monitor_reason_tank))
    }
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .onFocusChanged { focused = it.isFocused }
            .focusable()
            .background(if (focused) raisedSurface else surface, androidx.compose.foundation.shape.RoundedCornerShape(6.dp))
            .border(if (focused) 2.dp else 1.dp, if (focused) accent else Color.Transparent, androidx.compose.foundation.shape.RoundedCornerShape(6.dp))
            .padding(horizontal = 22.dp, vertical = 16.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(entry.prefix, modifier = Modifier.width(230.dp), color = textColor, fontSize = 26.sp, fontWeight = FontWeight.Bold)
        Column(modifier = Modifier.weight(1f)) {
            Text(stringResource(R.string.monitor_call_reasons), color = mutedText, fontSize = 14.sp)
            Text(reasons.joinToString("  |  ").ifEmpty { stringResource(R.string.monitor_unknown) }, color = if (entry.calledByManager) warning else textColor, fontSize = 19.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        }
        Column(horizontalAlignment = Alignment.End) {
            Text(stringResource(R.string.monitor_departure), color = mutedText, fontSize = 14.sp)
            Text(formatTimestamp(entry.nextDeparture), color = textColor, fontSize = 17.sp)
        }
    }
}

@Composable
private fun MatrixPage(
    matrix: GarageMatrix,
    textColor: Color,
    mutedText: Color,
    surface: Color,
    raisedSurface: Color,
    accent: Color,
) {
    if (matrix.columns.isEmpty() || matrix.rows.isEmpty()) {
        EmptyState(stringResource(R.string.monitor_empty_matrix), mutedText)
        return
    }
    val horizontalState = rememberScrollState()
    val verticalState = rememberScrollState()
    Column(
        modifier = Modifier
            .fillMaxSize()
            .horizontalScroll(horizontalState),
    ) {
        Column(modifier = Modifier.verticalScroll(verticalState)) {
            Row(
                modifier = Modifier.background(raisedSurface).padding(vertical = 12.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text("PREFIXO", modifier = Modifier.width(170.dp).padding(start = 16.dp), color = mutedText, fontSize = 15.sp, fontWeight = FontWeight.Bold)
                matrix.columns.forEach { column ->
                    Column(modifier = Modifier.width(190.dp).padding(horizontal = 10.dp)) {
                        Text(column.name, color = textColor, fontSize = 17.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(stringResource(if (column.internal) R.string.monitor_internal else R.string.monitor_external), color = mutedText, fontSize = 13.sp)
                    }
                }
            }
            matrix.rows.forEach { row ->
                var focused by remember(row.vehicleUuid) { androidx.compose.runtime.mutableStateOf(false) }
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .onFocusChanged { focused = it.isFocused }
                        .focusable()
                        .background(if (focused) raisedSurface else surface)
                        .border(if (focused) 2.dp else 1.dp, if (focused) accent else Color.Transparent)
                        .padding(vertical = 12.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(row.prefix, modifier = Modifier.width(170.dp).padding(start = 16.dp), color = textColor, fontSize = 20.sp, fontWeight = FontWeight.Bold)
                    matrix.columns.indices.forEach { index ->
                        val cell = row.cells.getOrNull(index)
                        var cellFocused by remember(row.vehicleUuid, index) { androidx.compose.runtime.mutableStateOf(false) }
                        Column(
                            modifier = Modifier
                                .width(190.dp)
                                .onFocusChanged { cellFocused = it.isFocused }
                                .focusable()
                                .background(if (cellFocused) accent.copy(alpha = 0.12f) else Color.Transparent)
                                .border(1.dp, if (cellFocused) accent else Color.Transparent)
                                .padding(horizontal = 10.dp, vertical = 4.dp),
                        ) {
                            Text(cell?.effectiveState?.let { serviceStateLabel(it) } ?: "—", color = accent, fontSize = 16.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text(
                                "${stringResource(R.string.monitor_matrix_stamp)} ${formatTimestamp(cell?.stamp)}",
                                color = mutedText,
                                fontSize = 12.sp,
                                maxLines = 1,
                            )
                            Text(
                                "${stringResource(R.string.monitor_matrix_last_performed)} ${formatTimestamp(cell?.lastPerformedAt)}",
                                color = mutedText,
                                fontSize = 12.sp,
                                maxLines = 1,
                            )
                        }
                    }
                }
            }
            if (matrix.rowsNotShown > 0 || matrix.columnsNotShown > 0) {
                Row(modifier = Modifier.padding(top = 12.dp), horizontalArrangement = Arrangement.spacedBy(18.dp)) {
                    if (matrix.rowsNotShown > 0) Text(stringResource(R.string.monitor_rows_not_shown, matrix.rowsNotShown), color = mutedText, fontSize = 15.sp)
                    if (matrix.columnsNotShown > 0) Text(stringResource(R.string.monitor_columns_not_shown, matrix.columnsNotShown), color = mutedText, fontSize = 15.sp)
                }
            }
        }
    }
}

@Composable
private fun DataLine(label: String, value: String?, textColor: Color, mutedText: Color) {
    Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(label, modifier = Modifier.weight(1f), color = mutedText, fontSize = 15.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Text(value ?: stringResource(R.string.monitor_unknown), color = textColor, fontSize = 16.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
private fun EmptyState(message: String, mutedText: Color) {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text(message, color = mutedText, fontSize = 24.sp, fontWeight = FontWeight.Medium)
    }
}

// shortcut: the API sends zone-less UTC timestamps (ICD has no zone field); upgrade if it ever emits an offset.
private fun formatTimestamp(value: String?): String {
    if (value == null) return "--"
    val parsed = runCatching {
        SimpleDateFormat("yyyy-MM-dd'T'HH:mm", Locale.ROOT)
            .apply { timeZone = TimeZone.getTimeZone("UTC") }
            .parse(value.take(16))
    }.getOrNull() ?: return value.replace('T', ' ').take(16)
    return SimpleDateFormat("yyyy-MM-dd HH:mm", Locale.ROOT).format(parsed)
}

@Composable
private fun serviceStateLabel(value: String): String = when (value) {
    "Pending" -> stringResource(R.string.monitor_state_pending)
    "Performed" -> stringResource(R.string.monitor_state_performed)
    "NotNeeded" -> stringResource(R.string.monitor_state_not_needed)
    "NotDone" -> stringResource(R.string.monitor_state_not_done)
    else -> value.uppercase(Locale("pt", "BR"))
}

private fun formatPercent(value: Double): String =
    String.format(Locale("pt", "BR"), "%.0f%%", value)

private fun formatDistance(value: Double): String =
    String.format(Locale("pt", "BR"), "%.0f km", value)

private fun readinessLabel(value: String?): String = when (value?.lowercase()) {
    "alert" -> "ALERTA"
    "preparing" -> "EM PREPARO"
    "ready" -> "PRONTO"
    else -> value?.uppercase(Locale("pt", "BR")) ?: "NÃO INFORMADO"
}