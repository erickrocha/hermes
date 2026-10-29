import { Fuel, Wrench, CalendarClock, Siren, AlertTriangle, type LucideIcon } from 'lucide-react'
import type { Session } from '../types'
import { can } from './access'
import { resourceConfigs } from './resources'

export interface NavItem { to: string; labelKey: string; icon: LucideIcon; visible: (session: Session) => boolean; groupKey?: string }

// Telas que não são cadastro genérico: ordens de serviço, combustível, garagem, escala, checklist, preventiva.
const custom: NavItem[] = [
  { to: '/work-orders', labelKey: 'workOrders', icon: Wrench, visible: (s) => can(s, 'work-order.read') },
  { to: '/non-executions', labelKey: 'nonExecutions', icon: AlertTriangle, visible: (s) => can(s, 'non-execution.read') },
  { to: '/fuel', labelKey: 'fuel', icon: Fuel, visible: (s) => can(s, 'fuel-entry.read') },
  { to: '/schedule', labelKey: 'schedule', icon: CalendarClock, visible: (s) => can(s, 'schedule.effective.read') },
  { to: '/garage', labelKey: 'garage', icon: Siren, visible: (s) => can(s, 'garage.queue.read') },
]

// Agrupamento do menu: o mesmo mapa serve a quem lê (cabeçalhos) e aos testes.
const GROUPS: Record<string, string> = {
  '/vehicles': 'navFleet', '/fuel': 'navFleet', '/vehicle-expenses': 'navFleet', '/checklist-templates': 'navFleet',
  '/schedule': 'navScheduling', '/transport-demands': 'navScheduling', '/extra-trips': 'navScheduling', '/customers': 'navScheduling', '/holidays': 'navScheduling',
  '/work-orders': 'navMaintenance', '/non-executions': 'navMaintenance', '/preventive-plans': 'navMaintenance', '/maintenance-windows': 'navMaintenance', '/inspection-models': 'navMaintenance', '/service-types': 'navMaintenance', '/priced-services': 'navMaintenance',
  '/garage': 'navGarage', '/garage-services': 'navGarage', '/parts': 'navStock', '/purchase-orders': 'navStock',
}
const ORDER = ['navFleet', 'navScheduling', 'navMaintenance', 'navGarage', 'navStock']

export const operationNav: NavItem[] = [
  ...custom,
  ...resourceConfigs.map((config) => ({ to: `/${config.route}`, labelKey: config.titleKey, icon: config.icon, visible: (s: Session) => config.read(s) })),
].map((item) => ({ ...item, groupKey: GROUPS[item.to] })).sort((a, b) => ORDER.indexOf(a.groupKey ?? '') - ORDER.indexOf(b.groupKey ?? ''))

// Raiz de cada seção para o rastro de navegação; as telas de recurso derivam o resto do próprio caminho.
export const sectionLabels: Record<string, string> = { ...Object.fromEntries(operationNav.map((item) => [item.to, item.labelKey])), '/inspections': 'recordInspection', '/cross-tenant-actions': 'crossTenantActions' }
