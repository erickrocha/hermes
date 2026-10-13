import { api } from '../api'
import { downloadFile, listPage } from './resourceService'
import type { Params } from './resourceService'

export interface NamedAmount { name: string; amount: number }
export interface VehicleCost {
  plate: string
  bySource: { workOrderPostingsCents: number; externalServicesCents: number; fuelCents: number; expensesCents: number }
  totalCostCents: number
  byServiceType: NamedAmount[]
  servicesPerformed: NamedAmount[]
  revenueCents: number
  resultCents?: number
}
export interface SupplierClosing {
  lines: { date: string; invoiceNumber?: string; service: string; workOrderType?: string; plate: string; invoiceCents: number }[]
  totalCents: number
}
export interface VehicleOption { uuid: string; plate: string; model?: string }

export const listCostVehicles = async (): Promise<VehicleOption[]> =>
  (await listPage<VehicleOption>('/vehicle', { page: 0, pageSize: 200 })).items

export const listSuppliers = async (): Promise<string[]> => (await api.get<string[]>('/cost-report/suppliers')).data

export const loadVehicleCost = async ({ vehicle, params }: { vehicle: string; params: Params }): Promise<VehicleCost> =>
  (await api.get<VehicleCost>(`/cost-report/vehicle/uuid/${vehicle}`, { params })).data

export const loadSupplierClosing = async (params: Params): Promise<SupplierClosing> =>
  (await api.get<SupplierClosing>('/cost-report/supplier-closing', { params })).data

export const downloadCostReport = async ({ path, params }: { path: string; params: Params }): Promise<void> =>
  downloadFile(path, params)