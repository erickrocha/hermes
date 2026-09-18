export type Role = 'SysAdmin' | 'TenantOwner' | 'TenantUser'

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
  firstLogin: boolean
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
  paymentGraceDays?: number
  businessPlanId?: number | null
}

export interface BusinessPlanTier { id?: number; upToUsers: number; pricePerUserInCents: number }
export interface BusinessPlan {
  id?: number
  uuid?: string
  name: string
  priceInCents: number
  availableUsers: number
  periodDays: number
  paymentDate: string
  tiers: BusinessPlanTier[]
}
export interface User {
  id?: number
  uuid?: string
  name?: string
  email: string
  password?: string
  enabled: boolean
  firstLogin: boolean
  role: Role
  tenantId?: number | null
}
export interface Province { id: number; acronym: string; name: string; countryCode: string }
export interface City { id: number; provinceId: number; name: string }
