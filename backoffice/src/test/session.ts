import type { ProfileScope, Session } from '../types'
import { CATALOGUE } from './catalogue'

export type SystemProfileKey = 'SysAdmin' | 'TenantOwner' | 'TenantUser' | 'Driver' | 'Mechanic'

const SCOPE_OF: Record<SystemProfileKey, ProfileScope> = { SysAdmin: 'Platform', TenantOwner: 'Tenant', TenantUser: 'Tenant', Driver: 'Tenant', Mechanic: 'Tenant' }

/// The grants of a system profile, derived from the catalogue mirror the way `system_grants` derives them server-side.
export const systemGrants = (key: SystemProfileKey): string[] => CATALOGUE.flatMap((p) => {
  switch (key) {
    case 'SysAdmin': return [p.code]
    case 'TenantOwner': return p.scope === 'Tenant' || p.scope === 'Baseline' ? [p.code] : []
    case 'TenantUser': return p.tenantUser ? [p.code] : []
    case 'Mechanic': return p.mechanic ? [p.code] : []
    case 'Driver': return p.driver ? [p.own ? `${p.code}@own` : p.code] : []
  }
})

export interface SessionOverrides extends Partial<Omit<Session, 'permissions' | 'scope'>> { tenantId?: number | null }

/// A session built from grants, the way the token now carries them. A tenant session sits in tenant 42 unless told otherwise.
export const sessionWith = (permissions: string[], scope: ProfileScope = 'Tenant', overrides: SessionOverrides = {}): Session => {
  const tenantId = overrides.tenantId === undefined ? (scope === 'Platform' ? null : 42) : overrides.tenantId
  return {
    accessToken: 't', tokenType: 'Bearer', expireIn: 3600, email: 'a@b.c', uuid: 'u', name: 'n', userId: 1,
    scope, permissions, tenantId, tenantUuid: tenantId ? `t-${tenantId}` : null, ...overrides,
  }
}

/// A session holding one of the five system profiles -- the former roles.
export const sessionAs = (key: SystemProfileKey, overrides: SessionOverrides = {}): Session =>
  sessionWith(systemGrants(key), SCOPE_OF[key], { profileKey: key, profileName: key, email: `${key}@transmega.com`, uuid: `${key}-uuid`, name: key, ...overrides })
