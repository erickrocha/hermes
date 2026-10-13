import { api } from '../api'
import type { BusinessPlan, City, Page, PageRequest, Province, Tenant, User } from '../types'

const pageParams = ({ page, pageSize, search }: PageRequest) => ({ params: { page, pageSize, search: search || undefined } })

export const fetchTenants = async (request: PageRequest): Promise<Page<Tenant>> =>
  (await api.get<Page<Tenant>>('/tenant', pageParams(request))).data
export const fetchPlans = async (request: PageRequest): Promise<Page<BusinessPlan>> =>
  (await api.get<Page<BusinessPlan>>('/business-plan', pageParams(request))).data
export const fetchUsers = async (request: PageRequest): Promise<Page<User>> =>
  (await api.get<Page<User>>('/user', pageParams(request))).data
export const fetchProvinces = async (countryCode: string): Promise<Province[]> =>
  (await api.get<Province[]>('/province', { params: { countryCode, country_code: countryCode } })).data
export const fetchCities = async (provinceId: number): Promise<City[]> =>
  (await api.get<City[]>(`/cities/by-province/${provinceId}`)).data
export const fetchCountries = async (): Promise<string[]> => (await api.get<string[]>('/country')).data

export const fetchCounts = async (includePlans: boolean) => {
  const countRequest = { params: { page: 0, pageSize: 1 } }
  const [tenants, users, plans] = await Promise.all([
    api.get<Page<Tenant>>('/tenant', countRequest),
    api.get<Page<User>>('/user', countRequest),
    includePlans ? api.get<Page<BusinessPlan>>('/business-plan', countRequest) : Promise.resolve(null),
  ])
  return { tenants: tenants.data.totalItems, users: users.data.totalItems, plans: plans ? plans.data.totalItems : 0 }
}

export const fetchCurrentTenant = async (tenantUuid: string): Promise<Tenant> =>
  (await api.get<Tenant>(`/tenant/uuid/${tenantUuid}`)).data