import type { ReactElement } from 'react'
import { can, OFFICE, OWNER, OWNER_MECHANIC } from './features/access'
import type { Role, Session } from './types'
import { CustomerDayOffPage, CustomerDetailPage, DemandAllocationPage, DemandDailyPage, DemandDetailPage, DemandExceptionPage, VehicleAssignPage, VehicleDetailPage, VehicleOdometerPage } from './pages/FleetPages'
import { ChecklistRunNewPage, ChecklistTemplateDetailPage, TechnicalInspectionNewPage } from './pages/ChecklistPages'
import { GarageCallCancelPage, GarageCallPage, GarageMonitorPage, GaragePage, GarageOpenPage, GarageTriagePage } from './pages/GaragePages'
import { GarageDevicesPage } from './pages/GarageDevicesPage'
import { PreventiveAlertPage, PreventiveExtensionPage, PreventivePlanDetailPage } from './pages/PreventivePages'
import { SchedulePage, TripImportPage } from './pages/SchedulePages'
import { FuelEntryNewPage, FuelGaugePage, FuelPage, FuelReceiptPage, FuelReportPage, FuelRestorePage, FuelSettingsPage, FuelUnifyPage, InternalTankConfigPage, InternalTankPage, RuleSettingsPage } from './pages/FuelPages'
import { MaintenanceNotificationsPage } from './pages/MaintenanceNotificationsPage'
import { canSeeMaintenanceNotifications } from './components/molecules/NotificationBell'
import { NonExecutionJustifyPage, NonExecutionManualPage, NonExecutionsPage, canSeeNonExecutions } from './pages/NonExecutionPages'
import { PartAdjustPage, PartDetailPage, PartEntryPage, PurchaseOrderDetailPage, PurchaseOrderOrderedPage } from './pages/StockPages'
import { WorkOrderDetailPage, WorkOrderIssuePartPage, WorkOrderItemPage, WorkOrderNewPage, WorkOrdersPage, canSeeWorkOrders } from './pages/WorkOrderPages'

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
