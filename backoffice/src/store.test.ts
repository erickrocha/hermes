import { describe, expect, it } from 'vitest'
import { normalizeSession } from './api'

// C-035: the token carries the session's scope and effective grants; `role` is gone.
describe('normalizeSession', () => {
  it('normalizes the backend camelCase token payload, grants included', () => {
    const session = normalizeSession({ accessToken: 'token', refreshToken: 'refresh', userId: 7, tenantId: 3, scope: 'Tenant', profileKey: 'TenantOwner', profileName: 'Tenant owner', permissions: ['vehicle.read', 'checklist-run.create@own'] })
    expect(session).toMatchObject({ accessToken: 'token', refreshToken: 'refresh', userId: 7, tenantId: 3, scope: 'Tenant', profileKey: 'TenantOwner', profileName: 'Tenant owner', permissions: ['vehicle.read', 'checklist-run.create@own'] })
    expect('role' in session).toBe(false)
  })
  it('keeps a platform session platform, without a tenant', () => {
    const session = normalizeSession({ accessToken: 'token', userId: 1, tenantId: null, scope: 'Platform', permissions: ['tenant.create'] })
    expect(session).toMatchObject({ scope: 'Platform', tenantId: null, profileKey: undefined, profileName: undefined })
  })
  it('supports legacy snake_case keys and treats a pre-C-035 stored session as a tenant session with no grants', () => {
    const session = normalizeSession({ access_token: 'token', user_id: 1, tenant_id: null, first_login: false, role: 'SysAdmin' })
    expect(session).toMatchObject({ accessToken: 'token', userId: 1, tenantId: null, scope: 'Tenant', permissions: [] })
    expect(session.name).toBe('')
    expect(session.email).toBe('')
  })
})
