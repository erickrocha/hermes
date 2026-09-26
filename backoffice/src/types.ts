export type Role = 'SysAdmin' | 'TenantOwner' | 'TenantUser' | 'Driver' | 'Mechanic'

export interface Session {
  accessToken: string
  refreshToken?: string
  tokenType: string
  expireIn: number
  email: string
  uuid: string
  name: string
  userId: number
  role: Role
  tenantId?: number | null
  tenantUuid?: string | null
}

export interface Tenant {
  id?: number
  uuid?: string
  businessName: string
  companyName?: string
  taxId: string
  email?: string
  phone?: string
  website?: string
  addressLine1?: string
  addressLine2?: string
  locality?: string
  administrativeArea?: string
  postalCode?: string
  countryCode?: string
  businessPlanId?: number | null
}

export interface BusinessPlan {
  id?: number
  uuid?: string
  name: string
  priceInCents: number
  availableUsers: number
  periodDays: number
  paymentDate: string
}
export interface User {
  id?: number
  uuid?: string
  name?: string
  email: string
  password?: string
  enabled: boolean
  role: Role
  tenantId?: number | null
}
export interface Province { id: number; acronym: string; name: string; countryCode: string }
export interface City { id: number; provinceId: number; name: string }

// PD-028: envelope de paginação do servidor. `page` é base zero, igual ao
// `pageIndex` do TanStack Table -- nada é convertido na fronteira.
export interface Page<T> {
  items: T[]
  page: number
  pageSize: number
  totalItems: number
  totalPages: number
}

export interface PageRequest {
  page: number
  pageSize: number
  /// PD-028: a busca é do servidor. Filtrar no cliente procuraria só dentro
  /// da página carregada e ignoraria o resto da tabela em silêncio.
  search?: string
}

export const emptyPage = <T,>(pageSize = 25): Page<T> => ({
  items: [], page: 0, pageSize, totalItems: 0, totalPages: 0,
})
