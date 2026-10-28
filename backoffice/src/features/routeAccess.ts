import type { Session } from '../types'
import { can, canPlatform, has, isPlatform } from './access'

// C-035: every guard names the permission the server enforces for the operation behind the screen
// (`backend/business/src/domain/operation_permission.rs`); platform-only areas also ask for the platform scope.
const USER_LIST_HOLDERS = ['user.create', 'user.update', 'user.invite', 'profile.assign']

export const canCreateUsers = (session: Session) => has(session, 'user.create')
/// `GET /user` answers whoever holds any of these; everybody else gets an empty page, so the screen is not offered.
export const canSeeUsers = (session: Session) => USER_LIST_HOLDERS.some((code) => has(session, code))
/// The tenant list: the platform runs every tenant; an owner finds their own tenant's card there.
export const canSeeTenants = (session: Session) => has(session, 'tenant.update')
export const canCreateTenants = (session: Session) => canPlatform(session, 'tenant.create')
export const canManageSubscription = (session: Session) => canPlatform(session, 'tenant.plan')
export const canSeePlans = (session: Session) => canPlatform(session, 'business-plan.read')
export const canSeeReferenceData = (session: Session) => canPlatform(session, 'reference-data.province.manage-read')
export const canSeeProfiles = (session: Session) => has(session, 'profile.read')
export const canEditProfiles = (session: Session) => has(session, 'profile.create') || has(session, 'profile.update')
export const canSeeCosts = (session: Session) => can(session, 'cost-report.vehicle.read')
export const canSeeBilling = (session: Session) => has(session, 'billing.read')
export const canAdministerBilling = (session: Session) => isPlatform(session) && (has(session, 'billing.confirm-payment') || has(session, 'billing.configure') || has(session, 'billing.suspend'))
export const canManageGarageDevices = (session: Session) => can(session, 'garage.device.manage')
export const canSeeWorkOrders = (session: Session) => can(session, 'work-order.read')
export const canSeeNonExecutions = (session: Session) => can(session, 'non-execution.read')
