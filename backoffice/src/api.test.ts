import { afterEach, describe, expect, it } from 'vitest'
import { api, normalizeSession, readTarget, TARGET_KEY } from './api'

const headersOf = async () => {
  let seen: Record<string, unknown> = {}
  await api.get('/probe', { adapter: async (config) => { seen = { ...config.headers.toJSON() }; return { data: {}, status: 200, statusText: 'OK', headers: {}, config } } })
  return seen
}

// C-036 (`DS-1`): while the console acts in a descendant tenant, every request names it.
describe('the tenant the console acts in', () => {
  afterEach(() => localStorage.clear())

  it('goes on every request as X-Tenant-Uuid, and stops when the console is back in its own tenant', async () => {
    localStorage.setItem(TARGET_KEY, JSON.stringify({ uuid: 'child-uuid', name: 'Filial Norte' }))
    expect((await headersOf())['X-Tenant-Uuid']).toBe('child-uuid')
    localStorage.removeItem(TARGET_KEY)
    expect((await headersOf())['X-Tenant-Uuid']).toBeUndefined()
  })

  it('reads as none when the stored value is missing or unreadable', () => {
    expect(readTarget()).toBeNull()
    localStorage.setItem(TARGET_KEY, '{not json')
    expect(readTarget()).toBeNull()
    localStorage.setItem(TARGET_KEY, JSON.stringify({ name: 'no uuid' }))
    expect(readTarget()).toBeNull()
  })

  it('keeps who may act in a descendant across a reload, even though inside one the permissions lack it', () => {
    const fresh = normalizeSession({ accessToken: 't', permissions: ['tenant.child.data.read'] })
    expect(fresh.canTarget).toBe(true)
    const insideChild = normalizeSession({ ...fresh, permissions: ['vehicle.read'] })
    expect(insideChild.canTarget).toBe(true)
    expect(normalizeSession({ accessToken: 't', permissions: ['vehicle.read'] }).canTarget).toBe(false)
  })
})
