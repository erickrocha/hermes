package com.erocha.hermes.client.tv

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.focusable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.onFocusChanged
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.erocha.hermes.client.shared.GarageCallListEntry
import com.erocha.hermes.client.shared.GarageMap
import com.erocha.hermes.client.shared.GarageMatrix
import com.erocha.hermes.client.shared.GarageMonitorCard
import com.erocha.hermes.client.shared.GarageMonitorEntry
import com.erocha.hermes.client.shared.themeColor
import androidx.compose.foundation.shape.RoundedCornerShape
import java.text.DateFormat
import java.util.Date
import java.text.SimpleDateFormat
import java.util.Locale
import java.util.TimeZone
import kotlin.math.ceil

internal data class GarageMonitorUiState(
    val queue: List<GarageMonitorEntry> = emptyList(),
    val cards: List<GarageMonitorCard> = emptyList(),
    val cardsNotShown: Int = 0,
    val callList: List<GarageCallListEntry> = emptyList(),
    val matrix: GarageMatrix = GarageMatrix(),
    /** `null` when the GPS provider did not answer. */
    val map: GarageMap? = null,
    val lastUpdatedAtMillis: Long? = null,
    val isRefreshing: Boolean = true,
    val errorMessage: String? = null,
)

@Composable
private fun themedColor(attr: Int) = Color(LocalContext.current.themeColor(attr))

private val CardShape = RoundedCornerShape(6.dp)
private val ChromeHeight = 36.dp

@Composable
internal fun GarageMonitorScreen(
    state: GarageMonitorUiState,
    onRefresh: () -> Unit,
    onRePair: () -> Unit,
    themeLabel: String,
    onTheme: () -> Unit,
) {
    val background = themedColor(R.attr.hermesMonitorBackground)
    val surface = themedColor(R.attr.hermesMonitorSurface)
    val raisedSurface = themedColor(R.attr.hermesMonitorSurfaceRaised)
    val textColor = themedColor(R.attr.hermesMonitorText)
    val mutedText = themedColor(R.attr.hermesMonitorTextMuted)
    val accent = themedColor(R.attr.hermesMonitorAccent)
    val warning = themedColor(R.attr.hermesMonitorWarning)
    val danger = themedColor(R.attr.hermesMonitorDanger)
    val outline = themedColor(R.attr.hermesMonitorOutline)
    var selectedTab by rememberSaveable { mutableIntStateOf(0) }
    val firstTabRequester = remember { FocusRequester() }

    // Back leaves a secondary tab for the first one; from the first, it leaves the app as usual.
    BackHandler(enabled = selectedTab != 0) {
        selectedTab = 0
        firstTabRequester.requestFocus()
    }

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
                .padding(horizontal = 32.dp, vertical = 14.dp),
        ) {
            Header(
                state = state,
                textColor = textColor,
                mutedText = mutedText,
                warning = warning,
                danger = danger,
            )
            Spacer(Modifier.height(10.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                val tabLabels = listOf(
                    stringResource(R.string.monitor_tab_queue),
                    stringResource(R.string.monitor_tab_fuel),
                    stringResource(R.string.monitor_tab_matrix),
                    stringResource(R.string.monitor_tab_calls),
                    stringResource(R.string.monitor_tab_map),
                )
                tabLabels.forEachIndexed { index, label ->
                    DashboardTab(
                        label = label,
                        selected = selectedTab == index,
                        selectedColor = accent,
                        textColor = textColor,
                        outlineColor = outline,
                        focusRequester = if (index == 0) firstTabRequester else null,
                        onSelect = { selectedTab = index },
                    )
                }
                Spacer(Modifier.weight(1f))
                DashboardAction(
                    label = stringResource(R.string.monitor_refresh),
                    enabled = !state.isRefreshing,
                    accent = accent,
                    textColor = textColor,
                    outlineColor = outline,
                    onClick = onRefresh,
                )
                DashboardAction(
                    label = stringResource(R.string.monitor_theme, themeLabel),
                    enabled = true,
                    accent = accent,
                    textColor = textColor,
                    outlineColor = outline,
                    onClick = onTheme,
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
            Spacer(Modifier.height(8.dp))
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .height(1.dp)
                    .background(outline),
            )
            Spacer(Modifier.height(8.dp))
            if (state.errorMessage != null) {
                Text(
                    text = state.errorMessage,
                    color = warning,
                    fontSize = 13.sp,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Spacer(Modifier.height(6.dp))
            }
            Box(modifier = Modifier.fillMaxSize().clipToBounds()) {
                when {
                    state.lastUpdatedAtMillis == null && state.isRefreshing ->
                        EmptyState(stringResource(R.string.monitor_loading), mutedText)
                    state.lastUpdatedAtMillis == null && state.errorMessage != null ->
                        EmptyState(state.errorMessage, warning)
                    else -> when (selectedTab) {
                        0 -> QueuePage(state.queue, textColor, mutedText, surface, raisedSurface, accent, warning, danger)
                        1 -> FuelPage(state.cards, state.cardsNotShown, textColor, mutedText, surface, raisedSurface, accent, warning)
                        2 -> MatrixPage(state.matrix, textColor, mutedText, surface, raisedSurface, accent)
                        3 -> CallsPage(state.callList, textColor, mutedText, surface, raisedSurface, accent, warning)
                        else -> MapPage(state.map, textColor, mutedText, surface, accent, warning, themedColor(R.attr.hermesMonitorOk), outline)
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
) {
    val hasSnapshot = state.lastUpdatedAtMillis != null
    val status = when {
        state.errorMessage != null -> stringResource(R.string.monitor_stale)
        state.isRefreshing && !hasSnapshot -> stringResource(R.string.monitor_loading)
        state.isRefreshing -> stringResource(R.string.monitor_updating)
        else -> stringResource(R.string.monitor_online)
    }
    val statusColor = if (state.errorMessage != null) warning else if (state.isRefreshing) warning else themedColor(R.attr.hermesMonitorOk)

    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Text(
                text = stringResource(R.string.monitor_title),
                color = textColor,
                fontSize = 24.sp,
                fontWeight = FontWeight.Bold,
            )
            Text(
                text = stringResource(R.string.monitor_subtitle),
                color = mutedText,
                fontSize = 14.sp,
                modifier = Modifier.padding(bottom = 3.dp),
            )
        }
        Spacer(Modifier.weight(1f))
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            state.lastUpdatedAtMillis?.let { updatedAt ->
                val time = remember(updatedAt) {
                    DateFormat.getTimeInstance(DateFormat.SHORT, Locale("pt", "BR"))
                        .format(Date(updatedAt))
                }
                Text(
                    text = stringResource(R.string.monitor_last_updated, time),
                    color = mutedText,
                    fontSize = 13.sp,
                )
            }
            Text(
                text = status,
                color = if (state.errorMessage != null) danger else statusColor,
                fontSize = 14.sp,
                fontWeight = FontWeight.SemiBold,
            )
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
    var focused by remember { mutableStateOf(false) }
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
            .height(ChromeHeight)
            .background(if (focused || selected) selectedColor.copy(alpha = 0.18f) else Color.Transparent, CardShape)
            .border(
                width = if (focused) 2.dp else 1.dp,
                color = if (focused || selected) selectedColor else outlineColor,
                shape = CardShape,
            )
            .padding(horizontal = 14.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            color = if (focused || selected) selectedColor else textColor,
            fontSize = 14.sp,
            fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
            maxLines = 1,
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
    var focused by remember { mutableStateOf(false) }
    Box(
        modifier = Modifier
            .onFocusChanged { focused = it.isFocused }
            .focusable(enabled = enabled)
            .clickable(enabled = enabled, onClick = onClick)
            .height(ChromeHeight)
            .background(if (focused) accent.copy(alpha = 0.18f) else Color.Transparent, CardShape)
            .border(
                width = if (focused) 2.dp else 1.dp,
                color = if (focused) accent else outlineColor,
                shape = CardShape,
            )
            .padding(horizontal = 14.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = label,
            color = if (enabled) textColor else textColor.copy(alpha = 0.45f),
            fontSize = 13.sp,
            fontWeight = FontWeight.SemiBold,
            maxLines = 1,
        )
    }
}

/**
 * Lays [items] out so that all of them are on screen at once: it adds columns (up to [maxColumns]) before it
 * lets a row shrink below [minRowHeight]. A monitor on a wall has nobody to scroll it, so scrolling only
 * remains as the last resort when even that does not fit.
 */
@Composable
private fun <T> FitGrid(
    items: List<T>,
    minRowHeight: Dp,
    maxRowHeight: Dp,
    modifier: Modifier = Modifier,
    maxColumns: Int = 3,
    gap: Dp = 6.dp,
    content: @Composable (T, Modifier) -> Unit,
) {
    BoxWithConstraints(modifier = modifier.fillMaxSize()) {
        val rowsThatFit = ((maxHeight + gap) / (minRowHeight + gap)).toInt().coerceAtLeast(1)
        val columns = ceil(items.size / rowsThatFit.toDouble()).toInt().coerceIn(1, maxColumns)
        val perColumn = ceil(items.size / columns.toDouble()).toInt().coerceAtLeast(1)
        val rowHeight = ((maxHeight - gap * (perColumn - 1)) / perColumn).coerceIn(minRowHeight, maxRowHeight)
        Row(
            modifier = Modifier.fillMaxWidth().verticalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(gap),
        ) {
            items.chunked(perColumn).forEach { chunk ->
                Column(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(gap)) {
                    chunk.forEach { item -> content(item, Modifier.fillMaxWidth().height(rowHeight)) }
                }
            }
            // A short last column keeps the others' width instead of stretching.
            repeat(columns - items.chunked(perColumn).size) { Spacer(Modifier.weight(1f)) }
        }
    }
}

@Composable
private fun focusCard(focused: Boolean, surface: Color, raisedSurface: Color, accent: Color): Modifier =
    Modifier
        .background(if (focused) raisedSurface else surface, CardShape)
        .border(if (focused) 2.dp else 1.dp, if (focused) accent else Color.Transparent, CardShape)

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
        Row(horizontalArrangement = Arrangement.spacedBy(20.dp)) {
            Text(stringResource(R.string.monitor_vehicle_count, entries.size), color = mutedText, fontSize = 13.sp)
            Text(
                stringResource(R.string.monitor_urgent_count, entries.count { it.urgent }),
                color = warning,
                fontSize = 13.sp,
                fontWeight = FontWeight.SemiBold,
            )
        }
        Spacer(Modifier.height(6.dp))
        FitGrid(entries, minRowHeight = 40.dp, maxRowHeight = 56.dp, modifier = Modifier.weight(1f)) { entry, rowModifier ->
            QueueRow(entry, rowModifier, textColor, mutedText, surface, raisedSurface, accent, warning, danger)
        }
    }
}

@Composable
private fun QueueRow(
    entry: GarageMonitorEntry,
    modifier: Modifier,
    textColor: Color,
    mutedText: Color,
    surface: Color,
    raisedSurface: Color,
    accent: Color,
    warning: Color,
    danger: Color,
) {
    var focused by remember { mutableStateOf(false) }
    val statusColor = when {
        entry.urgent -> danger
        entry.readiness.equals("Ready", ignoreCase = true) -> themedColor(R.attr.hermesMonitorOk)
        entry.readiness.equals("Alert", ignoreCase = true) -> warning
        else -> mutedText
    }
    Row(
        modifier = modifier
            .onFocusChanged { focused = it.isFocused }
            .focusable()
            .then(focusCard(focused, surface, raisedSurface, accent))
            .padding(horizontal = 12.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(modifier = Modifier.weight(1f)) {
            Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(entry.prefix, color = textColor, fontSize = 18.sp, fontWeight = FontWeight.Bold, maxLines = 1)
                if (entry.urgent) Text(stringResource(R.string.monitor_urgent), color = danger, fontSize = 11.sp, fontWeight = FontWeight.Bold)
                if (entry.calledByManager) Text(stringResource(R.string.monitor_called), color = warning, fontSize = 11.sp)
                if (entry.fillTankAlert) Text(stringResource(R.string.monitor_fill_tank), color = warning, fontSize = 11.sp)
            }
            Text(
                text = entry.requiredPending.takeIf { it.isNotEmpty() }?.joinToString(" | ")
                    ?: stringResource(R.string.monitor_unknown),
                color = mutedText,
                fontSize = 11.sp,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
        }
        Spacer(Modifier.width(8.dp))
        Column(horizontalAlignment = Alignment.End) {
            Text(readinessLabel(entry.readiness), color = statusColor, fontSize = 12.sp, fontWeight = FontWeight.SemiBold, maxLines = 1)
            Text(
                text = listOfNotNull(formatShort(entry.nextDeparture), entry.manualPriority?.let { "P$it" }).joinToString(" · "),
                color = if (entry.manualPriority == null) mutedText else accent,
                fontSize = 11.sp,
                maxLines = 1,
            )
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
        FitGrid(cards, minRowHeight = 92.dp, maxRowHeight = 150.dp, modifier = Modifier.weight(1f), gap = 10.dp) { card, cardModifier ->
            FuelCard(card, cardModifier, textColor, mutedText, surface, raisedSurface, accent, warning)
        }
        if (notShown > 0) {
            Text(stringResource(R.string.monitor_not_shown, notShown), color = mutedText, fontSize = 12.sp)
        }
    }
}

@Composable
private fun FuelCard(
    card: GarageMonitorCard,
    modifier: Modifier,
    textColor: Color,
    mutedText: Color,
    surface: Color,
    raisedSurface: Color,
    accent: Color,
    warning: Color,
) {
    var focused by remember { mutableStateOf(false) }
    Column(
        modifier = modifier
            .onFocusChanged { focused = it.isFocused }
            .focusable()
            .then(focusCard(focused, surface, raisedSurface, accent))
            .padding(horizontal = 14.dp, vertical = 8.dp),
        verticalArrangement = Arrangement.Center,
    ) {
        Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(card.prefix, color = textColor, fontSize = 20.sp, fontWeight = FontWeight.Bold, maxLines = 1)
            Text(stringResource(R.string.monitor_tank), color = mutedText, fontSize = 12.sp, modifier = Modifier.padding(bottom = 2.dp))
            Text(
                card.tankPercent?.let(::formatPercent) ?: stringResource(R.string.monitor_unknown),
                color = if (card.needsRefuel) warning else themedColor(R.attr.hermesMonitorOk),
                fontSize = 18.sp,
                fontWeight = FontWeight.Bold,
            )
            if (card.needsRefuel) Text(stringResource(R.string.monitor_needs_refuel), color = warning, fontSize = 12.sp, modifier = Modifier.padding(bottom = 2.dp))
        }
        Spacer(Modifier.height(4.dp))
        DataLine(stringResource(R.string.monitor_maintenance), card.maintenanceStatus, textColor, mutedText)
        DataLine(stringResource(R.string.monitor_last_cycle), card.lastGarageCycleDistanceKm?.let(::formatDistance), textColor, mutedText)
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
    FitGrid(entries, minRowHeight = 40.dp, maxRowHeight = 56.dp) { entry, rowModifier ->
        CallRow(entry, rowModifier, textColor, mutedText, surface, raisedSurface, accent, warning)
    }
}

@Composable
private fun CallRow(
    entry: GarageCallListEntry,
    modifier: Modifier,
    textColor: Color,
    mutedText: Color,
    surface: Color,
    raisedSurface: Color,
    accent: Color,
    warning: Color,
) {
    var focused by remember { mutableStateOf(false) }
    val reasons = buildList {
        if (entry.calledByManager) add(stringResource(R.string.monitor_reason_manager))
        if (entry.hasDepartureAhead) add(stringResource(R.string.monitor_reason_departure))
        if (entry.tankCalls) add(stringResource(R.string.monitor_reason_tank))
    }
    Row(
        modifier = modifier
            .onFocusChanged { focused = it.isFocused }
            .focusable()
            .then(focusCard(focused, surface, raisedSurface, accent))
            .padding(horizontal = 12.dp, vertical = 4.dp),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(entry.prefix, modifier = Modifier.width(76.dp), color = textColor, fontSize = 18.sp, fontWeight = FontWeight.Bold, maxLines = 1)
        Text(
            reasons.joinToString(" | ").ifEmpty { stringResource(R.string.monitor_unknown) },
            modifier = Modifier.weight(1f),
            color = if (entry.calledByManager) warning else textColor,
            fontSize = 12.sp,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Text(formatShort(entry.nextDeparture), color = mutedText, fontSize = 11.sp, maxLines = 1)
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
    val prefixWidth = 72.dp
    val headerHeight = 40.dp
    val gap = 3.dp
    BoxWithConstraints(modifier = Modifier.fillMaxSize()) {
        val footerHeight = if (matrix.rowsNotShown > 0 || matrix.columnsNotShown > 0) 18.dp else 0.dp
        val columnWidth = (maxWidth - prefixWidth) / matrix.columns.size
        val rowHeight = ((maxHeight - headerHeight - footerHeight - gap * matrix.rows.size) / matrix.rows.size).coerceIn(30.dp, 52.dp)
        Column(verticalArrangement = Arrangement.spacedBy(gap)) {
            Row(
                modifier = Modifier.fillMaxWidth().height(headerHeight).background(raisedSurface, CardShape),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(stringResource(R.string.monitor_matrix_prefix), modifier = Modifier.width(prefixWidth).padding(start = 8.dp), color = mutedText, fontSize = 11.sp, fontWeight = FontWeight.Bold, maxLines = 1)
                matrix.columns.forEach { column ->
                    Column(modifier = Modifier.width(columnWidth).padding(horizontal = 4.dp)) {
                        Text(column.name, color = textColor, fontSize = 12.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                        Text(stringResource(if (column.internal) R.string.monitor_internal else R.string.monitor_external), color = mutedText, fontSize = 10.sp, maxLines = 1)
                    }
                }
            }
            matrix.rows.forEach { row ->
                var focused by remember(row.vehicleUuid) { mutableStateOf(false) }
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .height(rowHeight)
                        .onFocusChanged { focused = it.isFocused }
                        .focusable()
                        .then(focusCard(focused, surface, raisedSurface, accent)),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(row.prefix, modifier = Modifier.width(prefixWidth).padding(start = 8.dp), color = textColor, fontSize = 15.sp, fontWeight = FontWeight.Bold, maxLines = 1)
                    matrix.columns.indices.forEach { index ->
                        val cell = row.cells.getOrNull(index)
                        var cellFocused by remember(row.vehicleUuid, index) { mutableStateOf(false) }
                        Column(
                            modifier = Modifier
                                .width(columnWidth)
                                .onFocusChanged { cellFocused = it.isFocused }
                                .focusable()
                                .background(if (cellFocused) accent.copy(alpha = 0.12f) else Color.Transparent)
                                .border(1.dp, if (cellFocused) accent else Color.Transparent)
                                .padding(horizontal = 4.dp, vertical = 2.dp),
                        ) {
                            Text(cell?.effectiveState?.let { serviceStateLabel(it) } ?: "—", color = accent, fontSize = 12.sp, fontWeight = FontWeight.SemiBold, maxLines = 1, overflow = TextOverflow.Ellipsis)
                            Text(formatShort(cell?.stamp), color = mutedText, fontSize = 10.sp, maxLines = 1)
                        }
                    }
                }
            }
            if (matrix.rowsNotShown > 0 || matrix.columnsNotShown > 0) {
                Row(horizontalArrangement = Arrangement.spacedBy(14.dp)) {
                    if (matrix.rowsNotShown > 0) Text(stringResource(R.string.monitor_rows_not_shown, matrix.rowsNotShown), color = mutedText, fontSize = 11.sp)
                    if (matrix.columnsNotShown > 0) Text(stringResource(R.string.monitor_columns_not_shown, matrix.columnsNotShown), color = mutedText, fontSize = 11.sp)
                }
            }
        }
    }
}

@Composable
private fun DataLine(label: String, value: String?, textColor: Color, mutedText: Color) {
    Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
        Text(label, modifier = Modifier.weight(1f), color = mutedText, fontSize = 12.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
        Text(value ?: stringResource(R.string.monitor_unknown), color = textColor, fontSize = 13.sp, maxLines = 1, overflow = TextOverflow.Ellipsis)
    }
}

@Composable
internal fun EmptyState(message: String, mutedText: Color) {
    Box(modifier = Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
        Text(message, color = mutedText, fontSize = 20.sp, fontWeight = FontWeight.Medium)
    }
}

// shortcut: the API sends zone-less UTC timestamps (ICD has no zone field); upgrade if it ever emits an offset.
// Compact "dd/MM HH:mm" for the dense layouts.
private fun formatShort(value: String?): String {
    if (value == null) return "--"
    val parsed = runCatching {
        SimpleDateFormat("yyyy-MM-dd'T'HH:mm", Locale.ROOT)
            .apply { timeZone = TimeZone.getTimeZone("UTC") }
            .parse(value.take(16))
    }.getOrNull() ?: return value.replace('T', ' ').take(16)
    return SimpleDateFormat("dd/MM HH:mm", Locale.ROOT).format(parsed)
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