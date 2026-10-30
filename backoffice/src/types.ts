export type ProfileScope = 'Platform' | 'Tenant'

/// C-035: a grant is a permission code (`vehicle.read`) or `code@own` -- over the holder's own records only.
export type Grant = string

export interface Session {
  accessToken: string
  refreshToken?: string
  tokenType: string
  expireIn: number
  email: string
  uuid: string
  name: string
  userId: number
  // C-035: the session's reach and effective grants come with the token; `GET /access-profile/mine` refreshes them.
  scope: ProfileScope
  profileKey?: string
  profileName?: string
  permissions: Grant[]
  tenantId?: number | null
  tenantUuid?: string | null
  // C-036: whether the account may act inside a descendant tenant (`tenant.child.data.read`, or the system administrator).
  canTarget?: boolean
}

/// C-036: the descendant tenant the console is acting in; every request carries it as `X-Tenant-Uuid`.
export interface TargetTenant { uuid: string; name: string }

/// C-036 (`HRMS-877`): one request a tenant made inside a descendant of its own.
export interface CrossTenantAction {
  uuid?: string
  actorUserId: number
  actorEmail: string
  actorTenantId: number
  affectedTenantId: number
  kind: 'read' | 'write'
  method: string
  operation: string
  path: string
  occurredAt: string
}

export interface Tenant {
  id?: number
  uuid?: string
  // C-036: the tenant that manages this one; absent for the root tenant.
  parentTenantId?: number | null
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

/// EPIC-SC-01-S05: a customer carries its identity (trade name, legal name, CPF/CNPJ) and address.
/// `name` is the trade name; the document is stored digits-only, checked by `countryCode`
/// (the tenant's country when omitted).
export interface Customer {
  id?: number
  uuid?: string
  tenantId?: number | null
  name: string
  legalName?: string | null
  taxId?: string | null
  addressLine1?: string | null
  addressLine2?: string | null
  locality?: string | null
  administrativeArea?: string | null
  postalCode?: string | null
  countryCode?: string | null
  status: string
  notes?: string | null
}

export interface BusinessPlan {
  id?: number
  uuid?: string
  name: string
  priceInCents: number
  pricePerExtraUserInCents: number
  availableUsers: number
  periodDays: number
}

export interface PublicPlan {
  name: string
  priceInCents: number
  pricePerExtraUserInCents: number
  availableUsers: number
  periodDays: number
}

export interface BillingCharge {
  uuid: string
  tenantUuid: string
  tenantName: string
  planName: string
  currency: 'BRL'
  periodStart: string
  periodEnd: string
  priceInCents: number
  pricePerExtraUserInCents: number
  includedUsers: number
  activeUsers: number
  extraUsers: number
  graceDays: number
  totalInCents: number
  status: 'Pending' | 'Paid'
  isOverdue: boolean
  paidAt?: string | null
  paidBy?: string | null
}

export interface BillingSettings {
  tenantUuid: string
  graceDays?: number | null
  cycleStartedAt?: string | null
  pendingPlan: boolean
  suspendedAt?: string | null
  suspendedBy?: string | null
}
export interface User {
  id?: number
  uuid?: string
  name?: string
  email: string
  password?: string
  enabled: boolean
  // C-035: the access profile is required on creation; the name, key and scope are read-only echoes.
  profileUuid?: string
  profileName?: string
  profileKey?: string
  scope?: ProfileScope
  extraPermissions: Grant[]
  /// Read-only: the effective grants (profile, extras and baseline) the server resolved for the account.
  permissions?: Grant[]
  tenantId?: number | null
  cpf?: string | null
}
export interface AccessProfile { uuid?: string; tenantId?: number | null; name: string; system: boolean; key?: string; active: boolean; permissions: Grant[] }
export interface Permission { code: string; module: string; scope: ProfileScope; own: boolean; defaultFor: string[] }
export interface EffectiveProfile { scope: ProfileScope; profileUuid?: string; profileName?: string; profileKey?: string; extraPermissions: Grant[]; permissions: Grant[] }
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
