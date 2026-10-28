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
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
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
import com.erocha.hermes.client.shared.Part
import com.erocha.hermes.client.shared.WorkOrder
import com.erocha.hermes.client.shared.WorkOrderItem
import com.erocha.hermes.client.shared.isOpen

/** The work orders of the day: the open ones, one order at a time, adding items and issuing parts. */
internal class MaintenanceState(private val host: FleetHost) {
    var orders by mutableStateOf<List<WorkOrder>?>(null)
    var vehicleNames by mutableStateOf<Map<String, String>>(emptyMap())
    var refreshing by mutableStateOf(false)
    var error by mutableStateOf<String?>(null)

    var open by mutableStateOf<WorkOrder?>(null)
    /** The uuid of what is being saved, so the polling never overwrites a call in flight. */
    var busy by mutableStateOf<String?>(null)

    var adding by mutableStateOf(false)
    var issuing by mutableStateOf(false)
    /** The item whose resolution is being written. */
    var resolving by mutableStateOf<WorkOrderItem?>(null)
    /** The last thing that went right, so the mechanic sees the baixa was recorded. */
    var notice by mutableStateOf<String?>(null)
    var parts by mutableStateOf<List<Part>?>(null)

    fun refresh(silent: Boolean = false) {
        if (!silent) refreshing = true
        host.runWorkshop({ api, token -> api.workOrders(token) }) { r ->
            refreshing = false
            r.onSuccess { list -> orders = list.filter { it.isOpen() }; if (open == null) error = null }
                .onFailure { if (!silent || orders == null) error = failureMessage(host, it, R.string.orders_loading_failed) }
        }
        if (vehicleNames.isEmpty()) {
            host.run({ api, token -> api.vehicles(token) }) { r ->
                r.onSuccess { list -> vehicleNames = list.associate { it.uuid to (it.prefix ?: it.plate) } }
            }
        }
        open?.uuid?.let { uuid ->
            host.runWorkshop({ api, token -> api.workOrder(token, uuid) }) { r ->
                r.onSuccess { if (open?.uuid == it.uuid && busy == null) open = it }
            }
        }
    }

    /** The list carries no items; the order is shown at once and its items arrive with the detail read. */
    fun show(order: WorkOrder) {
        open = order
        error = null
        val uuid = order.uuid ?: return
        host.runWorkshop({ api, token -> api.workOrder(token, uuid) }) { r ->
            r.onSuccess { if (open?.uuid == it.uuid) open = it }
                .onFailure { error = failureMessage(host, it, R.string.orders_loading_failed) }
        }
    }

    fun close() {
        open = null
        error = null
        notice = null
        refresh(silent = true)
    }

    fun nameOf(order: WorkOrder) = vehicleNames[order.vehicleUuid] ?: order.number.orEmpty()

    fun addItem(description: String) {
        val uuid = open?.uuid ?: return
        adding = false
        busy = uuid
        host.runWorkshop({ api, token -> api.addItem(token, uuid, description.trim()) }) { r ->
            busy = null
            r.onSuccess { open = it }.onFailure { error = failureMessage(host, it, R.string.orders_item_failed) }
        }
    }

    fun resolve(item: WorkOrderItem, description: String) {
        val orderUuid = open?.uuid ?: return
        val itemUuid = item.uuid ?: return
        resolving = null
        busy = orderUuid
        notice = null
        host.runWorkshop({ api, token -> api.resolveItem(token, orderUuid, itemUuid, description.trim()) }) { r ->
            busy = null
            r.onSuccess { open = it }.onFailure { error = failureMessage(host, it, R.string.orders_resolve_failed) }
        }
    }

    fun startIssuing() {
        issuing = true
        if (parts == null) {
            host.runWorkshop({ api, token -> api.parts(token) }) { r ->
                r.onSuccess { parts = it }.onFailure { issuing = false; error = failureMessage(host, it, R.string.orders_part_failed) }
            }
        }
    }

    fun issue(part: Part, quantity: Double, itemUuid: String?) {
        val uuid = open?.uuid ?: return
        val partUuid = part.uuid ?: return
        issuing = false
        notice = null
        busy = uuid
        host.runWorkshop({ api, token -> api.issuePart(token, uuid, partUuid, quantity, itemUuid) }) { r ->
            busy = null
            r.onSuccess {
                open = it
                notice = host.context.getString(R.string.orders_issued, part.name, quantity.toString().removeSuffix(".0"))
            }.onFailure { error = failureMessage(host, it, R.string.orders_part_failed) }
        }
    }
}

@Composable
private fun itemStatusLabel(status: String?): String = stringResource(
    when (status) {
        "Resolved" -> R.string.item_resolved
        "Cancelled" -> R.string.item_cancelled
        "AwaitingParts" -> R.string.item_awaiting_parts
        else -> R.string.item_pending
    },
)

@Composable
private fun itemStatusColor(status: String?): Color = when (status) {
    "Resolved" -> token(R.attr.hermesOk)
    "Cancelled" -> token(R.attr.hermesTextSecondary)
    else -> token(R.attr.hermesWarning)
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun OrdersTab(state: MaintenanceState) {
    val orders = state.orders
    Column(modifier = Modifier.fillMaxSize()) {
        state.error?.let { ErrorBanner(it) }
        PullToRefreshBox(isRefreshing = state.refreshing, onRefresh = { state.refresh() }, modifier = Modifier.fillMaxSize()) {
            LazyColumn(
                modifier = Modifier.fillMaxSize(),
                verticalArrangement = Arrangement.spacedBy(10.dp),
                contentPadding = PaddingValues(16.dp),
            ) {
                if (orders != null && orders.isEmpty()) {
                    item { Text(stringResource(R.string.orders_empty), color = token(R.attr.hermesTextSecondary), modifier = Modifier.padding(8.dp)) }
                }
                items(orders.orEmpty(), key = { it.uuid ?: it.number.orEmpty() }) { order ->
                    Column(
                        modifier = Modifier
                            .fillMaxWidth()
                            .background(token(R.attr.hermesSurface), RoundedCornerShape(16.dp))
                            .border(BorderStroke(1.dp, token(R.attr.hermesOutline)), RoundedCornerShape(16.dp))
                            .clickable { state.show(order) }
                            .padding(16.dp),
                    ) {
                        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
                            Text(state.nameOf(order), color = token(R.attr.hermesTextPrimary), fontSize = 24.sp, fontWeight = FontWeight.Bold)
                            Spacer(Modifier.weight(1f))
                            order.number?.let { Text(it, color = token(R.attr.hermesTextSecondary), fontSize = 13.sp) }
                        }
                        if (order.description.isNotBlank()) {
                            Text(order.description, color = token(R.attr.hermesTextPrimary), fontSize = 15.sp, maxLines = 2, modifier = Modifier.padding(top = 4.dp))
                        }
                    }
                }
            }
        }
    }
}

@Composable
internal fun ColumnScope.OrderScreen(state: MaintenanceState) {
    val order = state.open ?: return
    Row(modifier = Modifier.fillMaxWidth().padding(start = 8.dp, end = 16.dp, top = 12.dp), verticalAlignment = Alignment.CenterVertically) {
        TextButton(onClick = { state.close() }) { Text("‹ ${stringResource(R.string.garage_back)}", color = token(R.attr.hermesAccent), fontSize = 16.sp) }
        Spacer(Modifier.weight(1f))
        Text(state.nameOf(order), color = token(R.attr.hermesTextPrimary), fontSize = 30.sp, fontWeight = FontWeight.Bold)
    }
    Column(modifier = Modifier.padding(horizontal = 20.dp, vertical = 4.dp)) {
        order.number?.let { Text(it, color = token(R.attr.hermesTextSecondary), fontSize = 14.sp) }
        if (order.description.isNotBlank()) Text(order.description, color = token(R.attr.hermesTextPrimary), fontSize = 16.sp)
        val pending = order.items.count { it.status == "Pending" || it.status == "AwaitingParts" }
        Text(
            stringResource(R.string.orders_items_count, order.items.size, pending),
            color = if (pending > 0) token(R.attr.hermesWarning) else token(R.attr.hermesOk),
            fontSize = 14.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(top = 4.dp),
        )
    }
    state.error?.let { ErrorBanner(it) }
    state.notice?.let {
        Text(it, color = token(R.attr.hermesOk), fontSize = 14.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.padding(horizontal = 20.dp, vertical = 4.dp))
    }
    LazyColumn(modifier = Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(10.dp), contentPadding = PaddingValues(16.dp)) {
        items(order.items, key = { it.uuid ?: it.description }) { item -> ItemCard(item, enabled = state.busy == null) { state.resolving = item } }
    }
    Row(modifier = Modifier.fillMaxWidth().padding(16.dp), horizontalArrangement = Arrangement.spacedBy(10.dp)) {
        val busy = state.busy != null
        OutlinedButton(
            onClick = { state.adding = true }, enabled = !busy, modifier = Modifier.weight(1f).height(56.dp), shape = RoundedCornerShape(16.dp),
            border = BorderStroke(1.dp, token(R.attr.hermesAccent)),
        ) { Text(stringResource(R.string.orders_add_item), color = token(R.attr.hermesAccent), fontSize = 15.sp, maxLines = 1) }
        Button(
            onClick = { state.startIssuing() }, enabled = !busy, modifier = Modifier.weight(1f).height(56.dp), shape = RoundedCornerShape(16.dp),
            colors = ButtonDefaults.buttonColors(containerColor = token(R.attr.hermesAccent), contentColor = Color.White),
        ) { Text(stringResource(R.string.orders_issue_part), fontSize = 15.sp, maxLines = 1) }
    }
    state.resolving?.let { ResolveDialog(state, it) }
    if (state.adding) AddItemDialog(state)
    if (state.issuing) IssuePartDialog(state, order)
}

@Composable
private fun ItemCard(item: WorkOrderItem, enabled: Boolean, onResolve: () -> Unit) {
    val color = itemStatusColor(item.status)
    Column(
        modifier = Modifier
            .fillMaxWidth()
            .background(token(R.attr.hermesSurface), RoundedCornerShape(16.dp))
            .border(BorderStroke(1.dp, token(R.attr.hermesOutline)), RoundedCornerShape(16.dp))
            .padding(16.dp),
    ) {
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(item.description, color = token(R.attr.hermesTextPrimary), fontSize = 17.sp, fontWeight = FontWeight.SemiBold, modifier = Modifier.weight(1f))
            Pill(itemStatusLabel(item.status), color)
        }
        item.observation?.takeIf { it.isNotBlank() }?.let {
            Text(it, color = token(R.attr.hermesTextSecondary), fontSize = 14.sp, modifier = Modifier.padding(top = 4.dp))
        }
        item.resolutionDescription?.takeIf { it.isNotBlank() }?.let {
            Text(it, color = token(R.attr.hermesOk), fontSize = 14.sp, modifier = Modifier.padding(top = 4.dp))
        }
        if (item.status == "Pending" || item.status == "AwaitingParts") {
            OutlinedButton(
                onClick = onResolve, enabled = enabled, shape = RoundedCornerShape(12.dp),
                border = BorderStroke(1.dp, token(R.attr.hermesOk)), modifier = Modifier.padding(top = 10.dp).fillMaxWidth().height(48.dp),
            ) { Text(stringResource(R.string.orders_resolve), color = token(R.attr.hermesOk), fontSize = 15.sp, fontWeight = FontWeight.SemiBold) }
        }
    }
}

@Composable
private fun ResolveDialog(state: MaintenanceState, item: WorkOrderItem) {
    var text by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = { state.resolving = null },
        title = { Text(stringResource(R.string.orders_resolve_title)) },
        text = {
            Column {
                Text(item.description, fontSize = 14.sp, modifier = Modifier.padding(bottom = 8.dp))
                OutlinedTextField(
                    value = text, onValueChange = { text = it },
                    placeholder = { Text(stringResource(R.string.orders_resolution_hint)) }, modifier = Modifier.fillMaxWidth(),
                )
            }
        },
        confirmButton = { TextButton(onClick = { state.resolve(item, text) }, enabled = text.isNotBlank()) { Text(stringResource(R.string.orders_confirm)) } },
        dismissButton = { TextButton(onClick = { state.resolving = null }) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
private fun AddItemDialog(state: MaintenanceState) {
    var text by remember { mutableStateOf("") }
    AlertDialog(
        onDismissRequest = { state.adding = false },
        title = { Text(stringResource(R.string.orders_add_item)) },
        text = {
            OutlinedTextField(
                value = text, onValueChange = { text = it },
                placeholder = { Text(stringResource(R.string.orders_item_hint)) }, modifier = Modifier.fillMaxWidth(),
            )
        },
        confirmButton = { TextButton(onClick = { state.addItem(text) }, enabled = text.isNotBlank()) { Text(stringResource(R.string.orders_confirm)) } },
        dismissButton = { TextButton(onClick = { state.adding = false }) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
private fun IssuePartDialog(state: MaintenanceState, order: WorkOrder) {
    var query by remember { mutableStateOf("") }
    var part by remember { mutableStateOf<Part?>(null) }
    var quantity by remember { mutableStateOf("1") }
    var itemUuid by remember { mutableStateOf<String?>(null) }
    val shown = state.parts.orEmpty().filter {
        query.isBlank() || it.name.contains(query.trim(), ignoreCase = true) || (it.code ?: "").contains(query.trim(), ignoreCase = true)
    }
    val amount = quantity.replace(',', '.').toDoubleOrNull()
    AlertDialog(
        onDismissRequest = { state.issuing = false },
        title = { Text(stringResource(R.string.orders_pick_part)) },
        text = {
            Column {
                OutlinedTextField(
                    value = query, onValueChange = { query = it; part = null }, singleLine = true,
                    placeholder = { Text(stringResource(R.string.orders_part_search_hint)) }, modifier = Modifier.fillMaxWidth(),
                )
                Spacer(Modifier.height(8.dp))
                LazyColumn(modifier = Modifier.height(200.dp)) {
                    if (shown.isEmpty()) item { Text(stringResource(R.string.orders_no_parts)) }
                    items(shown, key = { it.uuid ?: it.name }) { candidate ->
                        val chosen = part?.uuid == candidate.uuid
                        Column(
                            modifier = Modifier
                                .fillMaxWidth()
                                .background(if (chosen) token(R.attr.hermesAccent).copy(alpha = 0.14f) else Color.Transparent, RoundedCornerShape(8.dp))
                                .clickable { part = candidate }
                                .padding(10.dp),
                        ) {
                            Text(candidate.name, fontSize = 16.sp, fontWeight = FontWeight.SemiBold)
                            candidate.currentStock?.let { Text(stringResource(R.string.orders_stock, it.toString().removeSuffix(".0"), candidate.unit), fontSize = 12.sp) }
                        }
                    }
                }
                OutlinedTextField(
                    value = quantity, onValueChange = { quantity = it }, singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal),
                    placeholder = { Text(stringResource(R.string.orders_quantity_hint)) }, modifier = Modifier.fillMaxWidth(),
                )
                if (order.items.isNotEmpty()) {
                    Text(stringResource(R.string.orders_for_item), fontSize = 12.sp, modifier = Modifier.padding(top = 8.dp))
                    LazyColumn(modifier = Modifier.height(96.dp)) {
                        item {
                            ItemChoice(stringResource(R.string.orders_no_item), itemUuid == null) { itemUuid = null }
                        }
                        items(order.items, key = { it.uuid ?: it.description }) { item ->
                            ItemChoice(item.description, itemUuid == item.uuid) { itemUuid = item.uuid }
                        }
                    }
                }
            }
        },
        confirmButton = {
            TextButton(
                onClick = { part?.let { state.issue(it, amount ?: 0.0, itemUuid) } },
                enabled = part != null && amount != null && amount > 0.0,
            ) { Text(stringResource(R.string.orders_confirm)) }
        },
        dismissButton = { TextButton(onClick = { state.issuing = false }) { Text(stringResource(R.string.cancel)) } },
    )
}

@Composable
private fun ItemChoice(label: String, selected: Boolean, onClick: () -> Unit) {
    Text(
        label,
        fontSize = 14.sp,
        fontWeight = if (selected) FontWeight.Bold else FontWeight.Normal,
        color = if (selected) token(R.attr.hermesAccent) else token(R.attr.hermesTextPrimary),
        modifier = Modifier.fillMaxWidth().clickable(onClick = onClick).padding(vertical = 6.dp),
    )
}
