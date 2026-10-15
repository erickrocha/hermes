import { describe, expect, it } from 'vitest'
import { normalizeSession } from './api'

describe('normalizeSession', () => {
  it('normalizes the backend camelCase token payload', () => {
    const session = normalizeSession({ accessToken: 'token', refreshToken: 'refresh', userId: 7, tenantId: 3, role: 'TenantOwner' })
    expect(session).toMatchObject({ accessToken: 'token', refreshToken: 'refresh', userId: 7, tenantId: 3, role: 'TenantOwner' })
  })
  it('supports legacy snake_case keys', () => {
    const session = normalizeSession({ access_token: 'token', user_id: 1, tenant_id: null, first_login: false, role: 'SysAdmin' })
    expect(session).toMatchObject({ accessToken: 'token', userId: 1, tenantId: null, role: 'SysAdmin' })
    expect(session.name).toBe('')
    expect(session.email).toBe('')
  })
})
