import { lazy, type ComponentType, type LazyExoticComponent, type ReactElement } from 'react'
import { can, OFFICE, OWNER, OWNER_MECHANIC } from './features/access'
import type { Role, Session } from './types'
import { MaintenanceNotificationsPage } from './pages/MaintenanceNotificationsPage'
import { canSeeMaintenanceNotifications } from './components/molecules/NotificationBell'
import { canSeeNonExecutions, canSeeWorkOrders } from './features/routeAccess'

export function lazyPage<Module extends object, Name extends keyof Module>(
  load: () => Promise<Module>,
  name: Name,
): LazyExoticComponent<ComponentType<any>> {
  return lazy(async () => ({ default: (await load())[name] as ComponentType<any> }))
}

const CustomerDayOffPage = lazyPage(() => import('./pages/FleetPages'), 'CustomerDayOffPage')
const CustomerDetailPage = lazyPage(() => import('./pages/FleetPages'), 'CustomerDetailPage')
const DemandAllocationPage = lazyPage(() => import('./pages/FleetPages'), 'DemandAllocationPage')
const DemandDailyPage = lazyPage(() => import('./pages/FleetPages'), 'DemandDailyPage')
const DemandDetailPage = lazyPage(() => import('./pages/FleetPages'), 'DemandDetailPage')
const DemandExceptionPage = lazyPage(() => import('./pages/FleetPages'), 'DemandExceptionPage')
const VehicleAssignPage = lazyPage(() => import('./pages/FleetPages'), 'VehicleAssignPage')
const VehicleDetailPage = lazyPage(() => import('./pages/FleetPages'), 'VehicleDetailPage')
const VehicleOdometerPage = lazyPage(() => import('./pages/FleetPages'), 'VehicleOdometerPage')
const ChecklistRunNewPage = lazyPage(() => import('./pages/ChecklistPages'), 'ChecklistRunNewPage')
const ChecklistTemplateDetailPage = lazyPage(() => import('./pages/ChecklistPages'), 'ChecklistTemplateDetailPage')
const TechnicalInspectionNewPage = lazyPage(() => import('./pages/ChecklistPages'), 'TechnicalInspectionNewPage')
const GarageCallCancelPage = lazyPage(() => import('./pages/GaragePages'), 'GarageCallCancelPage')
const GarageCallPage = lazyPage(() => import('./pages/GaragePages'), 'GarageCallPage')
const GarageMonitorPage = lazyPage(() => import('./pages/GaragePages'), 'GarageMonitorPage')
const GaragePage = lazyPage(() => import('./pages/GaragePages'), 'GaragePage')
const GarageOpenPage = lazyPage(() => import('./pages/GaragePages'), 'GarageOpenPage')
const GarageTriagePage = lazyPage(() => import('./pages/GaragePages'), 'GarageTriagePage')
const GarageDevicesPage = lazyPage(() => import('./pages/GarageDevicesPage'), 'GarageDevicesPage')
const PreventiveAlertPage = lazyPage(() => import('./pages/PreventivePages'), 'PreventiveAlertPage')
const PreventiveExtensionPage = lazyPage(() => import('./pages/PreventivePages'), 'PreventiveExtensionPage')
const PreventivePlanDetailPage = lazyPage(() => import('./pages/PreventivePages'), 'PreventivePlanDetailPage')
const SchedulePage = lazyPage(() => import('./pages/SchedulePages'), 'SchedulePage')
const TripImportPage = lazyPage(() => import('./pages/SchedulePages'), 'TripImportPage')
const FuelEntryNewPage = lazyPage(() => import('./pages/FuelPages'), 'FuelEntryNewPage')
const FuelGaugePage = lazyPage(() => import('./pages/FuelPages'), 'FuelGaugePage')
const FuelPage = lazyPage(() => import('./pages/FuelPages'), 'FuelPage')
const FuelReceiptPage = lazyPage(() => import('./pages/FuelPages'), 'FuelReceiptPage')
const FuelReportPage = lazyPage(() => import('./pages/FuelPages'), 'FuelReportPage')
const FuelRestorePage = lazyPage(() => import('./pages/FuelPages'), 'FuelRestorePage')
const FuelSettingsPage = lazyPage(() => import('./pages/FuelPages'), 'FuelSettingsPage')
const FuelUnifyPage = lazyPage(() => import('./pages/FuelPages'), 'FuelUnifyPage')
const InternalTankConfigPage = lazyPage(() => import('./pages/FuelPages'), 'InternalTankConfigPage')
const InternalTankPage = lazyPage(() => import('./pages/FuelPages'), 'InternalTankPage')
const RuleSettingsPage = lazyPage(() => import('./pages/FuelPages'), 'RuleSettingsPage')
const NonExecutionJustifyPage = lazyPage(() => import('./pages/NonExecutionPages'), 'NonExecutionJustifyPage')
const NonExecutionManualPage = lazyPage(() => import('./pages/NonExecutionPages'), 'NonExecutionManualPage')
const NonExecutionsPage = lazyPage(() => import('./pages/NonExecutionPages'), 'NonExecutionsPage')
const PartAdjustPage = lazyPage(() => import('./pages/StockPages'), 'PartAdjustPage')
const PartDetailPage = lazyPage(() => import('./pages/StockPages'), 'PartDetailPage')
const PartEntryPage = lazyPage(() => import('./pages/StockPages'), 'PartEntryPage')
const PurchaseOrderDetailPage = lazyPage(() => import('./pages/StockPages'), 'PurchaseOrderDetailPage')
const PurchaseOrderOrderedPage = lazyPage(() => import('./pages/StockPages'), 'PurchaseOrderOrderedPage')
const WorkOrderDetailPage = lazyPage(() => import('./pages/WorkOrderPages'), 'WorkOrderDetailPage')
const WorkOrderIssuePartPage = lazyPage(() => import('./pages/WorkOrderPages'), 'WorkOrderIssuePartPage')
const WorkOrderItemPage = lazyPage(() => import('./pages/WorkOrderPages'), 'WorkOrderItemPage')
const WorkOrderNewPage = lazyPage(() => import('./pages/WorkOrderPages'), 'WorkOrderNewPage')
const WorkOrdersPage = lazyPage(() => import('./pages/WorkOrderPages'), 'WorkOrdersPage')

const READERS: Role[] = ['TenantOwner', 'TenantUser', 'Mechanic']
export interface AppRoute { path: string; element: ReactElement; allow: (session: Session) => boolean }

// As telas que não são cadastro genérico. Cada rota carrega a permissão que a protege (a mesma do botão que leva até ela).
export const customRoutes: AppRoute[] = [
  { path: '/work-orders', element: <WorkOrdersPage />, allow: canSeeWorkOrders },
  { path: '/work-orders/new', element: <WorkOrderNewPage />, allow: (s) => can(s, 'work-order.create', OWNER_MECHANIC) },
  { path: '/work-orders/:uuid', element: <WorkOrderDetailPage />, allow: canSeeWorkOrders },
  { path: '/work-orders/:uuid/items/new', element: <WorkOrderItemPage />, allow: (s) => can(s, 'work-order.item.add', OWNER_MECHANIC) },
  { path: '/work-orders/:uuid/issue-part', element: <WorkOrderIssuePartPage />, allow: (s) => can(s, 'work-order.issue-part', OWNER_MECHANIC) },
  { path: '/non-executions', element: <NonExecutionsPage />, allow: canSeeNonExecutions },
  { path: '/non-executions/manual', element: <NonExecutionManualPage />, allow: (s) => can(s, 'non-execution.record-manual', OWNER_MECHANIC) },
  { path: '/non-executions/:uuid/justify', element: <NonExecutionJustifyPage />, allow: (s) => can(s, 'non-execution.justify', OWNER_MECHANIC) },
  { path: '/maintenance-notifications', element: <MaintenanceNotificationsPage />, allow: canSeeMaintenanceNotifications },
  { path: '/vehicles/:uuid', element: <VehicleDetailPage />, allow: (s) => can(s, 'vehicle.read', OFFICE) },
  { path: '/vehicles/:uuid/odometer', element: <VehicleOdometerPage />, allow: (s) => can(s, 'vehicle.odometer.record', OWNER) },
  { path: '/vehicles/:uuid/assign', element: <VehicleAssignPage />, allow: (s) => can(s, 'vehicle.assign-driver', OWNER) },
  { path: '/customers/:uuid', element: <CustomerDetailPage />, allow: (s) => can(s, 'customer.read', OFFICE) },
  { path: '/customers/:uuid/day-offs/new', element: <CustomerDayOffPage />, allow: (s) => can(s, 'customer.day-off.create', OWNER) },
  { path: '/transport-demands/:uuid', element: <DemandDetailPage />, allow: (s) => can(s, 'transport-demand.read', OFFICE) },
  { path: '/transport-demands/:uuid/allocations/new', element: <DemandAllocationPage />, allow: (s) => can(s, 'transport-demand.allocation.create', OWNER) },
  { path: '/transport-demands/:uuid/daily-schedules/new', element: <DemandDailyPage />, allow: (s) => can(s, 'transport-demand.daily-schedule.create', OWNER) },
  { path: '/transport-demands/:uuid/exceptions/new', element: <DemandExceptionPage />, allow: (s) => can(s, 'transport-demand.exception.create', OWNER) },
  { path: '/fuel', element: <FuelPage />, allow: (s) => can(s, 'fuel-entry.read', READERS) },
  { path: '/fuel/new', element: <FuelEntryNewPage />, allow: (s) => can(s, 'fuel-entry.create', OWNER_MECHANIC) },
  { path: '/fuel/receipt', element: <FuelReceiptPage />, allow: (s) => can(s, 'fuel-entry.register-receipt', [...OWNER_MECHANIC, 'Driver']) },
  { path: '/fuel/unify', element: <FuelUnifyPage />, allow: (s) => can(s, 'fuel-entry.unify', OWNER_MECHANIC) },
  { path: '/fuel/restore', element: <FuelRestorePage />, allow: (s) => can(s, 'fuel-entry.restore', OWNER_MECHANIC) },
  { path: '/fuel/report', element: <FuelReportPage />, allow: (s) => can(s, 'fuel.report.read', OFFICE) },
  { path: '/fuel/gauge', element: <FuelGaugePage />, allow: (s) => can(s, 'fuel.tank-gauge.read', OFFICE) },
  { path: '/fuel/tank', element: <InternalTankPage />, allow: (s) => can(s, 'internal-tank.read', OFFICE) },
  { path: '/fuel/tank/edit', element: <InternalTankConfigPage />, allow: (s) => can(s, 'internal-tank.update', OWNER) },
  { path: '/fuel/settings', element: <FuelSettingsPage />, allow: (s) => can(s, 'fuel-gauge-settings.update', OWNER) },
  { path: '/fuel/rules', element: <RuleSettingsPage />, allow: (s) => can(s, 'tenant-rule-settings.update', OWNER) },
  { path: '/checklist-templates/:uuid', element: <ChecklistTemplateDetailPage />, allow: (s) => can(s, 'checklist-template.read', OFFICE) },
  { path: '/checklist-templates/:uuid/run', element: <ChecklistRunNewPage />, allow: (s) => can(s, 'checklist-run.create', [...OWNER, 'Driver']) },
  { path: '/inspections/new', element: <TechnicalInspectionNewPage />, allow: (s) => can(s, 'technical-inspection.create', OWNER_MECHANIC) },
  { path: '/preventive-plans/:uuid', element: <PreventivePlanDetailPage />, allow: (s) => can(s, 'preventive-plan.read', OFFICE) },
  { path: '/preventive-plans/:uuid/extension', element: <PreventiveExtensionPage />, allow: (s) => can(s, 'preventive-plan.extend', OWNER_MECHANIC) },
  { path: '/preventive-plans/:uuid/alerts/new', element: <PreventiveAlertPage />, allow: (s) => can(s, 'preventive-plan.alert.read', OWNER_MECHANIC) },
  { path: '/garage', element: <GaragePage />, allow: (s) => can(s, 'garage.queue.read', READERS) },
  { path: '/garage/open', element: <GarageOpenPage />, allow: (s) => can(s, 'garage.triage.open', OWNER_MECHANIC) },
  { path: '/garage/call', element: <GarageCallPage />, allow: (s) => can(s, 'garage.call.create', OWNER_MECHANIC) },
  { path: '/garage/call/cancel', element: <GarageCallCancelPage />, allow: (s) => can(s, 'garage.call.cancel', OWNER_MECHANIC) },
  { path: '/garage/monitor', element: <GarageMonitorPage />, allow: (s) => can(s, 'garage.monitor.read', READERS) },
  { path: '/garage/devices', element: <GarageDevicesPage />, allow: (s) => s.role === 'TenantOwner' },
  { path: '/garage/triage/:uuid', element: <GarageTriagePage />, allow: (s) => can(s, 'garage.triage.read', READERS) },
  { path: '/schedule', element: <SchedulePage />, allow: (s) => can(s, 'schedule.effective.read', OFFICE) },
  { path: '/extra-trips/import', element: <TripImportPage />, allow: (s) => can(s, 'extra-trip.import', OWNER) },
  { path: '/parts/:uuid', element: <PartDetailPage />, allow: (s) => can(s, 'part.read', OFFICE) },
  { path: '/parts/:uuid/entry', element: <PartEntryPage />, allow: (s) => can(s, 'part.stock-entry', OWNER_MECHANIC) },
  { path: '/parts/:uuid/adjustment', element: <PartAdjustPage />, allow: (s) => can(s, 'part.stock-adjust', OWNER_MECHANIC) },
  { path: '/purchase-orders/:uuid', element: <PurchaseOrderDetailPage />, allow: (s) => can(s, 'purchase-order.read', OFFICE) },
  { path: '/purchase-orders/:uuid/ordered', element: <PurchaseOrderOrderedPage />, allow: (s) => can(s, 'purchase-order.mark-ordered', OWNER_MECHANIC) },
]
