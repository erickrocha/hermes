import { describe, expect, it } from 'vitest'
import { can, canPlatform, grantCode, has, hasFull, isOwnGrant, isPlatform, onlyOwn, ownGrant } from './access'
import { canCreateTenants, canSeeBilling, canSeePlans, canSeeProfiles, canSeeReferenceData, canSeeTenants } from './routeAccess'
import { sessionAs, sessionWith, systemGrants } from '../test/session'

// C-035: the console reads the session's grants the way the server does -- `code` or `code@own` both answer for
// `code`, only the first answers in full, and platform areas also ask for the platform scope.
describe('permission helpers', () => {
  const driver = sessionWith(['vehicle.read@own', 'driver.act'])
  const owner = sessionWith(['vehicle.read', 'vehicle.create'])

  it('has: a grant or its @own narrowing both answer for the code', () => {
    expect(has(driver, 'vehicle.read')).toBe(true)
    expect(has(owner, 'vehicle.read')).toBe(true)
    expect(has(driver, 'vehicle.create')).toBe(false)
    expect(has(null, 'vehicle.read')).toBe(false)
  })

  it('hasFull / onlyOwn: tell the full permission from the own-records one', () => {
    expect(hasFull(owner, 'vehicle.read')).toBe(true)
    expect(hasFull(driver, 'vehicle.read')).toBe(false)
    expect(onlyOwn(driver, 'vehicle.read')).toBe(true)
    expect(onlyOwn(owner, 'vehicle.read')).toBe(false)
    expect(onlyOwn(driver, 'vehicle.create')).toBe(false)
  })

  it('isPlatform follows the scope, not the tenant', () => {
    expect(isPlatform(sessionWith([], 'Platform'))).toBe(true)
    expect(isPlatform(sessionWith([], 'Tenant', { tenantId: null }))).toBe(false)
    expect(isPlatform(null)).toBe(false)
  })

  it('can: a tenant operation needs the grant inside a tenant; canPlatform: the grant with platform reach', () => {
    expect(can(owner, 'vehicle.read')).toBe(true)
    expect(can(sessionWith(['vehicle.read'], 'Platform'), 'vehicle.read')).toBe(false)
    expect(canPlatform(sessionWith(['business-plan.read'], 'Platform'), 'business-plan.read')).toBe(true)
    expect(canPlatform(sessionWith(['business-plan.read']), 'business-plan.read')).toBe(false)
    expect(canPlatform(sessionWith([], 'Platform'), 'business-plan.read')).toBe(false)
  })

  it('grant strings round-trip', () => {
    expect(ownGrant('vehicle.read')).toBe('vehicle.read@own')
    expect(grantCode('vehicle.read@own')).toBe('vehicle.read')
    expect(grantCode('vehicle.read')).toBe('vehicle.read')
    expect(isOwnGrant('vehicle.read@own')).toBe(true)
    expect(isOwnGrant('vehicle.read')).toBe(false)
  })
})

describe('system profiles through the route guards', () => {
  it('the Driver profile carries the owner rulings over own records, and may act as a driver', () => {
    const grants = systemGrants('Driver')
    for (const grant of ['checklist-run.create@own', 'vehicle.odometer.record@own', 'fuel-entry.register-receipt@own', 'driver.act']) expect(grants).toContain(grant)
    expect(onlyOwn(sessionAs('Driver'), 'vehicle.read')).toBe(true)
  })

  it('platform areas open to the platform administrator alone', () => {
    const admin = sessionAs('SysAdmin'); const owner = sessionAs('TenantOwner')
    for (const guard of [canSeePlans, canSeeReferenceData, canCreateTenants]) { expect(guard(admin)).toBe(true); expect(guard(owner)).toBe(false) }
  })

  it('tenant administration opens to the owner, not to the mechanic or the driver', () => {
    for (const guard of [canSeeTenants, canSeeProfiles, canSeeBilling]) {
      expect(guard(sessionAs('TenantOwner'))).toBe(true)
      expect(guard(sessionAs('Mechanic'))).toBe(false)
      expect(guard(sessionAs('Driver'))).toBe(false)
    }
  })
})
