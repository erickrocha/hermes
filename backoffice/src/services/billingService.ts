import { api } from '../api'
import type { BillingCharge, BillingSettings, Page, PageRequest } from '../types'

export const loadBillingCharges = async (request: PageRequest): Promise<Page<BillingCharge>> =>
  (await api.get<Page<BillingCharge>>('/billing/charges', { params: request })).data

export const loadBillingSettings = async (tenantUuid: string): Promise<BillingSettings> =>
  (await api.get<BillingSettings>(`/billing/tenant/uuid/${tenantUuid}/settings`)).data

export const saveBillingSettings = async ({ tenantUuid, graceDays }: { tenantUuid: string; graceDays: number }): Promise<BillingSettings> =>
  (await api.put<BillingSettings>(`/billing/tenant/uuid/${tenantUuid}/settings`, { graceDays })).data

export const setBillingSuspension = async ({ tenantUuid, suspended }: { tenantUuid: string; suspended: boolean }): Promise<BillingSettings> =>
  (await api.put<BillingSettings>(`/billing/tenant/uuid/${tenantUuid}/suspension`, { suspended })).data

export const confirmBillingCharge = async (uuid: string): Promise<BillingCharge> =>
  (await api.post<BillingCharge>(`/billing/charges/uuid/${uuid}/confirm`)).data