import type { Role, Session, User } from '../types'
import { can } from './access'

export const canCreateUsers = (role: string) => role === 'SysAdmin' || role === 'TenantOwner'
export const tenantOwnerCreatableRoles: User['role'][] = ['TenantUser', 'Driver', 'Mechanic']
export const canReachAdministration = (role: string) => role !== 'Driver' && role !== 'Mechanic'
export const canManageTenant = (role: string) => role === 'SysAdmin' || role === 'TenantOwner'
export const canManageProfiles = (role: string) => role === 'TenantOwner'

const holds = (session: Session, code: string, byRole: boolean) =>
  session.permissions ? session.permissions.includes(code) : byRole

export const canSeeUsers = (session: Session) =>
  holds(session, 'user.create', canReachAdministration(session.role))
export const canSeeTenants = (session: Session) =>
  holds(session, 'tenant.update', canManageTenant(session.role))
export const canSeeProfiles = (session: Session) =>
  session.role !== 'SysAdmin' && holds(session, 'profile.read', canManageProfiles(session.role))

export const canSeeCosts = (session: Session) =>
  session.role !== 'SysAdmin' && (session.permissions
    ? session.permissions.includes('cost-report.vehicle.read')
    : session.role === 'TenantOwner' || session.role === 'TenantUser')

export const canSeeBilling = (session: Session) =>
  session.role === 'SysAdmin' || session.role === 'TenantOwner'

const workOrderReaders: Role[] = ['TenantOwner', 'TenantUser', 'Mechanic']
const nonExecutionReaders: Role[] = ['TenantOwner', 'TenantUser', 'Mechanic']

export const canSeeWorkOrders = (session: Session) =>
  can(session, 'work-order.read', workOrderReaders)
export const canSeeNonExecutions = (session: Session) =>
  can(session, 'non-execution.read', nonExecutionReaders)