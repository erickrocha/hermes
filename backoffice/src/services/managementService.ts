import { api } from '../api'
import type { BusinessPlan, Page, Tenant, User } from '../types'

export interface SubscriptionDetails {
  tenant: Tenant
  plans: BusinessPlan[]
  currentPlan: BusinessPlan | null
}

export const loadTenant = async (uuid: string): Promise<Tenant> => (await api.get<Tenant>(`/tenant/uuid/${uuid}`)).data

export const saveTenant = async ({ uuid, tenant }: { uuid?: string; tenant: Tenant }): Promise<void> => {
  if (uuid) await api.put(`/tenant/uuid/${uuid}`, tenant)
  else await api.post('/tenant', tenant)
}

export const loadSubscription = async (uuid: string): Promise<SubscriptionDetails> => {
  const [tenant, plans, currentPlan] = await Promise.all([
    api.get<Tenant>(`/tenant/uuid/${uuid}`),
    api.get<Page<BusinessPlan>>('/business-plan', { params: { page: 0, pageSize: 200 } }),
    api.get<BusinessPlan | null>(`/tenant/uuid/${uuid}/plan`).catch(() => null),
  ])
  return { tenant: tenant.data, plans: plans.data.items ?? [], currentPlan: currentPlan?.data ?? null }
}

export const saveSubscription = async ({ uuid, businessPlanId }: { uuid: string; businessPlanId: number }): Promise<void> => {
  await api.post(`/tenant/uuid/${uuid}/plan`, { businessPlanId })
}

export const deletePlan = async (uuid: string): Promise<void> => { await api.delete(`/business-plan/uuid/${uuid}`) }
export const loadPlan = async (uuid: string): Promise<BusinessPlan> => (await api.get<BusinessPlan>(`/business-plan/uuid/${uuid}`)).data

export const savePlan = async ({ uuid, plan }: { uuid?: string; plan: BusinessPlan }): Promise<void> => {
  if (uuid) await api.put(`/business-plan/uuid/${uuid}`, plan)
  else await api.post('/business-plan', plan)
}

export const setUserEnabled = async ({ user, enabled }: { user: User; enabled: boolean }): Promise<void> => {
  if (!user.uuid) throw new Error('User UUID is required')
  await api.put(`/user/uuid/${user.uuid}`, { ...user, password: undefined, enabled })
}

export const loadUser = async (uuid: string): Promise<User> => (await api.get<User>(`/user/uuid/${uuid}`)).data
export const resendUserInvite = async (uuid: string): Promise<void> => { await api.post(`/user/uuid/${uuid}/invite`) }

export const saveUser = async ({ uuid, user }: { uuid?: string; user: User }): Promise<void> => {
  const payload = { ...user }
  delete payload.password
  if (uuid) await api.put(`/user/uuid/${uuid}`, payload)
  else await api.post('/user', payload)
}