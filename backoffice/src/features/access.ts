import type { Role, Session } from '../types'

/// Um botão ou uma entrada de menu segue a permissão que o servidor impõe; sem o perfil carregado, cai no papel.
export const can = (session: Session | null, code: string, roles: Role[]) =>
  Boolean(session) && session!.role !== 'SysAdmin' && (session!.permissions ? session!.permissions.includes(code) : roles.includes(session!.role))

export const OFFICE: Role[] = ['TenantOwner', 'TenantUser', 'Mechanic', 'Driver']
export const OWNER: Role[] = ['TenantOwner']
export const OWNER_MECHANIC: Role[] = ['TenantOwner', 'Mechanic']
