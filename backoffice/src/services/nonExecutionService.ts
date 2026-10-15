import { api } from '../api'
import type { Page } from '../types'

/// C-026 EPIC-MT-04: planned maintenance that was not carried out.
export interface NonExecutionOccurrence {
  uuid: string
  kind: 'EnteredAndLeft' | 'DidNotAppear' | 'Manual'
  windowStart: string
  windowEnd: string
  arrivedAt?: string | null
  departedAt?: string | null
  dwellMinutes?: number | null
  minutesInWindow?: number | null
  justification?: string | null
  justifiedBy?: string | null
  justifiedAt?: string | null
}
export interface NonExecutionWorkOrder {
  workOrderUuid: string
  description: string
  status: string
  totalOccurrences: number
  occurrences: NonExecutionOccurrence[]
}
export interface NonExecutionVehicleGroup {
  vehicle: { uuid: string; plate: string; model: string }
  workOrders: NonExecutionWorkOrder[]
}
export interface NonExecutionEvaluation { plansEvaluated: number; occurrencesRecorded: number; plansConcluded: number }

export const listInbox = async (page: number, pageSize: number): Promise<Page<NonExecutionVehicleGroup>> =>
  (await api.get<Page<NonExecutionVehicleGroup>>('/non-execution', { params: { page, pageSize } })).data

export const justifyOccurrence = async (uuid: string, justification: string): Promise<NonExecutionOccurrence> =>
  (await api.put<NonExecutionOccurrence>(`/non-execution/occurrence/uuid/${uuid}/justify`, { justification })).data

export const recordManual = async (workOrderUuid: string, justification: string): Promise<NonExecutionOccurrence> =>
  (await api.post<NonExecutionOccurrence>('/non-execution/manual', { workOrderUuid, justification })).data

export const evaluateNow = async (): Promise<NonExecutionEvaluation> =>
  (await api.post<NonExecutionEvaluation>('/non-execution/evaluate')).data
