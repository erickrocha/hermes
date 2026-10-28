package com.erocha.hermes.client.shared

import io.ktor.client.HttpClient
import io.ktor.client.call.body
import io.ktor.client.request.bearerAuth
import io.ktor.client.request.get
import io.ktor.client.request.parameter
import io.ktor.client.request.post
import io.ktor.client.request.put
import io.ktor.client.request.setBody
import io.ktor.http.ContentType
import io.ktor.http.contentType
import kotlinx.serialization.Serializable

/** A valeta checklist the tenant defined (`GET /inspection-model`). */
@Serializable
data class InspectionModel(
    val uuid: String? = null,
    val name: String,
    val generatesWorkOrder: Boolean = true,
    val active: Boolean = true,
    val items: List<String> = emptyList(),
)

@Serializable
data class InspectionAnswer(
    val description: String,
    val conforming: Boolean,
    val observation: String? = null,
)

@Serializable
data class TechnicalInspectionRequest(
    val vehicleUuid: String,
    val inspectionModel: String,
    val inspectedAt: String? = null,
    val odometerKm: Double? = null,
    val observation: String? = null,
    val items: List<InspectionAnswer>,
)

@Serializable
data class TechnicalInspectionResult(
    val uuid: String? = null,
    val workOrderOpened: Boolean = false,
    val workOrderUuids: List<String> = emptyList(),
)

@Serializable
data class WorkOrderItem(
    val uuid: String? = null,
    val description: String,
    val itemType: String? = null,
    /** `Pending`, `Resolved`, `Cancelled` or `AwaitingParts`. */
    val status: String? = null,
    val observation: String? = null,
    val resolutionDescription: String? = null,
)

@Serializable
data class WorkOrder(
    val uuid: String? = null,
    val number: String? = null,
    val vehicleUuid: String,
    val openedAt: String? = null,
    val description: String = "",
    val status: String? = null,
    val items: List<WorkOrderItem> = emptyList(),
    val totalCostCents: Long? = null,
)

@Serializable
data class Part(
    val uuid: String? = null,
    val code: String? = null,
    val name: String,
    val unit: String = "",
    val currentStock: Double? = null,
    val location: String? = null,
)

@Serializable
private data class Resolution(val resolutionDescription: String)

@Serializable
private data class IssuePart(val partUuid: String, val quantity: Double, val workOrderItemUuid: String? = null)

const val ORDER_CONCLUDED = "Concluded"
const val ORDER_CANCELLED = "Cancelled"

/**
 * The workshop routes the mechanic uses on the phone: the valeta inspection (`HRMS-330s`) and the work orders of
 * the day (`HRMS-700s`). It only transports and decodes; every rule is the server's.
 */
class WorkshopApi(
    private val httpClient: HttpClient,
    private val baseUrl: String,
) {
    private fun url(path: String) = "${baseUrl.trimEnd('/')}$path"

    suspend fun inspectionModels(accessToken: String): List<InspectionModel> =
        httpClient.get(url("/inspection-model")) {
            bearerAuth(accessToken)
            parameter("page", 0)
            parameter("pageSize", MODELS_LIMIT)
        }.body<Page<InspectionModel>>().items.filter { it.active }

    suspend fun submitInspection(accessToken: String, request: TechnicalInspectionRequest): TechnicalInspectionResult =
        httpClient.post(url("/technical-inspection")) {
            bearerAuth(accessToken)
            contentType(ContentType.Application.Json)
            setBody(request)
        }.body()

    /** The most recently opened work orders; the open ones are what the mechanic works. */
    suspend fun workOrders(accessToken: String): List<WorkOrder> =
        httpClient.get(url("/work-order")) {
            bearerAuth(accessToken)
            parameter("page", 0)
            parameter("pageSize", ORDERS_LIMIT)
        }.body<Page<WorkOrder>>().items

    suspend fun workOrder(accessToken: String, uuid: String): WorkOrder =
        httpClient.get(url("/work-order/uuid/$uuid")) { bearerAuth(accessToken) }.body()

    suspend fun addItem(accessToken: String, orderUuid: String, description: String): WorkOrder {
        httpClient.post(url("/work-order/uuid/$orderUuid/item")) {
            bearerAuth(accessToken)
            contentType(ContentType.Application.Json)
            setBody(WorkOrderItem(description = description, status = "Pending"))
        }
        return workOrder(accessToken, orderUuid)
    }

    /** Settles one pendency, saying what was done; the answer is the order as it reads now. */
    suspend fun resolveItem(accessToken: String, orderUuid: String, itemUuid: String, description: String): WorkOrder {
        httpClient.put(url("/work-order/uuid/$orderUuid/item/uuid/$itemUuid/resolve")) {
            bearerAuth(accessToken)
            contentType(ContentType.Application.Json)
            setBody(Resolution(description))
        }
        return workOrder(accessToken, orderUuid)
    }

    suspend fun parts(accessToken: String): List<Part> =
        httpClient.get(url("/part")) {
            bearerAuth(accessToken)
            parameter("page", 0)
            parameter("pageSize", PARTS_LIMIT)
        }.body<Page<Part>>().items

    suspend fun issuePart(accessToken: String, orderUuid: String, partUuid: String, quantity: Double, itemUuid: String?): WorkOrder {
        httpClient.post(url("/work-order/uuid/$orderUuid/issue-part")) {
            bearerAuth(accessToken)
            contentType(ContentType.Application.Json)
            setBody(IssuePart(partUuid, quantity, itemUuid))
        }
        return workOrder(accessToken, orderUuid)
    }

    private companion object {
        const val MODELS_LIMIT = 50
        const val ORDERS_LIMIT = 50
        const val PARTS_LIMIT = 200
    }
}

/** An order the mechanic still works: neither concluded nor cancelled. */
fun WorkOrder.isOpen(): Boolean = status != ORDER_CONCLUDED && status != ORDER_CANCELLED
