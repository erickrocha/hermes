import type { Grant, Session } from '../types'

export const OWN_SUFFIX = '@own'

/// A grant is `code` or `code@own`; both answer for `code` -- the server narrows the second one to the holder's own records.
export const has = (session: Session | null | undefined, code: string) =>
  Boolean(session?.permissions?.some((grant) => grant === code || grant === `${code}${OWN_SUFFIX}`))

/// The permission over every record of the tenant, not just the holder's own.
export const hasFull = (session: Session | null | undefined, code: string) => Boolean(session?.permissions?.includes(code))

/// Held over the holder's own records only: the screen narrows what it lists and offers.
export const onlyOwn = (session: Session | null | undefined, code: string) => has(session, code) && !hasFull(session, code)

/// A platform administrator: a Platform-scope profile and no tenant of their own.
export const isPlatform = (session: Session | null | undefined) => session?.scope === 'Platform'

/// A tenant operation (vehicles, work orders, fuel…): the permission the server asks for, held inside a tenant.
/// A platform actor has no tenant to operate in, so these screens stay out of their menu even though they hold every code.
export const can = (session: Session | null | undefined, code: string) => !isPlatform(session) && has(session, code)

/// A platform-only area (tenants, business plans, reference data, billing administration): the permission AND the platform scope.
export const canPlatform = (session: Session | null | undefined, code: string) => isPlatform(session) && has(session, code)

export const grantCode = (grant: Grant) => (grant.endsWith(OWN_SUFFIX) ? grant.slice(0, -OWN_SUFFIX.length) : grant)
export const isOwnGrant = (grant: Grant) => grant.endsWith(OWN_SUFFIX)
export const ownGrant = (code: string) => `${code}${OWN_SUFFIX}`
