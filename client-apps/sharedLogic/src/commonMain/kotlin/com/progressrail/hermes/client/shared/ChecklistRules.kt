package com.progressrail.hermes.client.shared

/** What the driver can do next with a vehicle, read from the server's possession answer (presentation only). */
sealed interface VehicleAction {
    data object Depart : VehicleAction
    data class Return(val openingChecklistUuid: String) : VehicleAction
    data object HeldByAnother : VehicleAction
}

fun vehicleAction(possession: ChecklistRun?, driverUuid: String): VehicleAction = when {
    possession == null -> VehicleAction.Depart
    possession.driverUuid == driverUuid && possession.uuid != null -> VehicleAction.Return(possession.uuid)
    else -> VehicleAction.HeldByAnother
}

/** The tenant's default template of [checklistType] (`SR-CA-03-09`); null when the owner has not set one. */
fun defaultTemplate(templates: List<ChecklistTemplate>, checklistType: String): ChecklistTemplate? =
    templates.firstOrNull { it.checklistType == checklistType && it.active && it.isDefault }

/** Accepts "12345", "12345.6" and "12345,6"; null when it is not a number. The server validates the value. */
fun parseOdometer(text: String): Double? = text.trim().replace(',', '.').toDoubleOrNull()?.takeIf { it.isFinite() }

/** "Enviar" is enabled only with a reading and every item answered (owner decision, 2026-10-15). */
fun canSubmitChecklist(odometerText: String, items: List<ChecklistTemplateItem>, answers: Map<String, String>): Boolean =
    parseOdometer(odometerText) != null && items.isNotEmpty() && items.all { answers[it.uuid] != null }

fun buildChecklistRun(
    template: ChecklistTemplate,
    driverUuid: String,
    vehicleUuid: String,
    action: VehicleAction,
    odometerKm: Double,
    answers: Map<String, String>,
    observations: Map<String, String>,
    notes: String,
): ChecklistRun = ChecklistRun(
    checklistTemplateUuid = template.uuid,
    driverUuid = driverUuid,
    vehicleUuid = vehicleUuid,
    checklistType = template.checklistType,
    odometerKm = odometerKm,
    notes = notes.trim().ifEmpty { null },
    openingChecklistUuid = (action as? VehicleAction.Return)?.openingChecklistUuid,
    answers = template.items.mapNotNull { item ->
        answers[item.uuid]?.let { status ->
            ChecklistAnswer(item.uuid, status, observations[item.uuid]?.trim()?.ifEmpty { null })
        }
    },
)
