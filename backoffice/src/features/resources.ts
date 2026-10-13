import type { LucideIcon } from 'lucide-react'
import { Bus, CalendarClock, CalendarDays, ClipboardCheck, ListChecks, SearchCheck, Package, ReceiptText, ShoppingCart, Tags, Users2, Wrench, MapPinned, Warehouse, Route as RouteIcon } from 'lucide-react'
import type { Session } from '../types'
import { can, OWNER, OWNER_MECHANIC, OFFICE } from './access'
import type { FieldDef } from './fields'
import type { Resource } from './hooks'
import { createResourceSlice } from './resourceSlice'

export type Row = Record<string, unknown> & { uuid?: string }
export type ColumnKind = 'text' | 'money' | 'date' | 'datetime' | 'bool' | 'badge' | 'number'
export interface ColumnSpec { key: string; labelKey: string; kind?: ColumnKind; prefix?: string }

/// Uma tela de cadastro por recurso, descrita em dados: o mesmo organismo desenha a lista, o cadastro e a edição.
export interface ResourceConfig {
  resource: Resource<Row>; route: string; titleKey: string; summaryKey: string; newKey: string; editKey: string; icon: LucideIcon
  fields: FieldDef[]; columns: ColumnSpec[]
  read: (session: Session | null) => boolean
  create?: (session: Session | null) => boolean
  edit?: (session: Session | null) => boolean
  remove?: (session: Session | null) => boolean
  /** A primeira coluna leva à página de detalhe `/<rota>/<uuid>`. */
  detail?: boolean
  /** Atalhos extras na barra da lista (importar, registrar inspeção…), cada um com a permissão que o protege. */
  shortcuts?: { to: string; labelKey: string; visible: (session: Session | null) => boolean }[]
}

const slice = (name: string, path: string) => createResourceSlice<Row>(name, path) as unknown as Resource<Row>

export const vehicles = slice('vehicles', '/vehicle')
export const serviceTypes = slice('serviceTypes', '/service-type')
export const pricedServices = slice('pricedServices', '/priced-service')
export const parts = slice('parts', '/part')
export const vehicleExpenses = slice('vehicleExpenses', '/vehicle-expense')
export const customers = slice('customers', '/customer')
export const holidays = slice('holidays', '/holiday')
export const transportDemands = slice('transportDemands', '/transport-demand')
export const extraTrips = slice('extraTrips', '/extra-trip')
export const garageServices = slice('garageServices', '/garage-service')
export const purchaseOrders = slice('purchaseOrders', '/purchase-order')
export const checklistTemplates = slice('checklistTemplates', '/checklist-template')
export const preventivePlans = slice('preventivePlans', '/preventive-plan')
export const workOrders = slice('workOrders', '/work-order')
export const inspectionModels = slice('inspectionModels', '/inspection-model')
export const maintenanceWindows = slice('maintenanceWindows', '/maintenance-plan')
export const technicalInspections = slice('technicalInspections', '/technical-inspection')
export const garageAttendances = slice('garageAttendances', '/garage-attendance')
export const checklistRuns = slice('checklistRuns', '/checklist-run')
export const fuelEntries = slice('fuelEntries', '/fuel-entry')
export const fuelGaugeSettings = slice('fuelGaugeSettings', '/fuel-gauge-settings')
export const ruleSettings = slice('ruleSettings', '/tenant-rule-settings')
export const internalTank = slice('internalTank', '/internal-tank')

const text = (name: string, labelKey: string, required = false): FieldDef => ({ name, labelKey, type: 'text', required })
const num = (name: string, labelKey: string, required = false, min?: number): FieldDef => ({ name, labelKey, type: 'number', required, min })
const date = (name: string, labelKey: string, required = false): FieldDef => ({ name, labelKey, type: 'date', required })
const pick = (name: string, labelKey: string, options: string[], optionPrefix: string, required = false): FieldDef => ({ name, labelKey, type: 'select', options, optionPrefix, required })

export const resourceConfigs: ResourceConfig[] = [
  {
    resource: vehicles, route: 'vehicles', detail: true, titleKey: 'vehicles', summaryKey: 'vehiclesSummary', newKey: 'newVehicle', editKey: 'editVehicle', icon: Bus,
    fields: [text('plate', 'plate', true), text('model', 'model', true), pick('status', 'status', ['Active', 'Maintenance', 'Transit', 'Reserved', 'Inactive'], 'vehicleStatus', true),
      text('prefix', 'prefix'), text('vehicleType', 'vehicleType'), text('wheelType', 'wheelType'), num('spareTireCount', 'spareTireCount', false, 0), text('spareTireType', 'spareTireType'),
      text('spareTireNotes', 'spareTireNotes'), num('tankCapacityLiters', 'tankCapacityLiters', false, 0), num('referenceKmPerLiter', 'referenceKmPerLiter', false, 0),
      pick('garageTagOrigin', 'garageTagOrigin', ['Manual', 'Tracker', 'Automatic'], 'tagOrigin'), text('garageTag', 'garageTag')],
    columns: [{ key: 'plate', labelKey: 'plate' }, { key: 'model', labelKey: 'model' }, { key: 'prefix', labelKey: 'prefix' }, { key: 'odometerKm', labelKey: 'odometerKm', kind: 'number' }, { key: 'status', labelKey: 'status', kind: 'badge', prefix: 'vehicleStatus' }],
    read: (s) => can(s, 'vehicle.read', OFFICE), create: (s) => can(s, 'vehicle.create', OWNER), edit: (s) => can(s, 'vehicle.update', OWNER),
  },
  {
    resource: customers, route: 'customers', detail: true, titleKey: 'customers', summaryKey: 'customersSummary', newKey: 'newCustomer', editKey: 'editCustomer', icon: Users2,
    fields: [text('name', 'name', true), pick('status', 'status', ['Active', 'Inactive'], 'customerStatus', true), { name: 'notes', labelKey: 'observation', type: 'textarea' }],
    columns: [{ key: 'name', labelKey: 'name' }, { key: 'status', labelKey: 'status', kind: 'badge', prefix: 'customerStatus' }],
    read: (s) => can(s, 'customer.read', OFFICE), create: (s) => can(s, 'customer.create', OWNER), edit: (s) => can(s, 'customer.update', OWNER),
  },
  {
    resource: holidays, route: 'holidays', titleKey: 'holidays', summaryKey: 'holidaysSummary', newKey: 'newHoliday', editKey: 'newHoliday', icon: CalendarDays,
    fields: [date('date', 'date', true), text('name', 'name', true)],
    columns: [{ key: 'date', labelKey: 'date', kind: 'date' }, { key: 'name', labelKey: 'name' }],
    read: (s) => can(s, 'holiday.read', OFFICE), create: (s) => can(s, 'holiday.create', OWNER), remove: (s) => can(s, 'holiday.delete', OWNER),
  },
  {
    resource: transportDemands, route: 'transport-demands', detail: true, titleKey: 'transportDemands', summaryKey: 'transportDemandsSummary', newKey: 'newDemand', editKey: 'editDemand', icon: RouteIcon,
    fields: [text('demandType', 'demandType', true), pick('demandKind', 'demandKind', ['Line', 'ExtraLine', 'OneOffTrip'], 'demandKind'), { name: 'customerUuid', labelKey: 'customer', type: 'lookup', lookup: 'customer' },
      text('lineName', 'lineName'), { name: 'shiftStart', labelKey: 'shiftStart', type: 'time' }, { name: 'shiftEnd', labelKey: 'shiftEnd', type: 'time' }, text('daysOfWeek', 'daysOfWeek'), date('specificDate', 'specificDate'),
      num('priority', 'priority', false, 0), text('preferredVehicleType', 'preferredVehicleType'), text('preferredVehicleModel', 'preferredVehicleModel'),
      { name: 'specificDriverUuid', labelKey: 'driver', type: 'lookup', lookup: 'driver' }, { name: 'specificVehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle' }, { name: 'active', labelKey: 'activeFlag', type: 'checkbox', defaultValue: true }],
    columns: [{ key: 'lineName', labelKey: 'lineName' }, { key: 'demandKind', labelKey: 'demandKind', kind: 'badge', prefix: 'demandKind' }, { key: 'daysOfWeek', labelKey: 'daysOfWeek' }, { key: 'specificDate', labelKey: 'specificDate', kind: 'date' }, { key: 'active', labelKey: 'activeFlag', kind: 'bool' }],
    read: (s) => can(s, 'transport-demand.read', OFFICE), create: (s) => can(s, 'transport-demand.create', OWNER), edit: (s) => can(s, 'transport-demand.update', OWNER),
  },
  {
    resource: extraTrips, route: 'extra-trips', titleKey: 'extraTrips', summaryKey: 'extraTripsSummary', newKey: 'newTrip', editKey: 'editTrip', icon: MapPinned,
    fields: [text('orderCode', 'orderCode', true), date('tripDate', 'tripDate', true), { name: 'customerUuid', labelKey: 'customer', type: 'lookup', lookup: 'customer' }, { name: 'startTime', labelKey: 'startTime', type: 'time' },
      date('returnDate', 'returnDate'), { name: 'returnTime', labelKey: 'returnTime', type: 'time' }, text('originCity', 'originCity'), text('destination', 'destination'), text('stops', 'stops'), text('preferredVehicleType', 'preferredVehicleType'),
      { name: 'driverUuid', labelKey: 'driver', type: 'lookup', lookup: 'driver' }, { name: 'secondDriverUuid', labelKey: 'secondDriver', type: 'lookup', lookup: 'driver' }, { name: 'vehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle' },
      { name: 'freightValueCents', labelKey: 'freightValue', type: 'money', min: 0 }, text('payment', 'payment'), pick('status', 'status', ['Scheduled', 'Conflict', 'PendingSchedule', 'Cancelled'], 'tripStatus', true)],
    columns: [{ key: 'orderCode', labelKey: 'orderCode' }, { key: 'tripDate', labelKey: 'tripDate', kind: 'date' }, { key: 'destination', labelKey: 'destination' }, { key: 'freightValueCents', labelKey: 'freightValue', kind: 'money' }, { key: 'status', labelKey: 'status', kind: 'badge', prefix: 'tripStatus' }],
    read: (s) => can(s, 'extra-trip.read', OFFICE), create: (s) => can(s, 'extra-trip.create', OWNER), edit: (s) => can(s, 'extra-trip.update', OWNER),
    shortcuts: [{ to: '/extra-trips/import', labelKey: 'importTrips', visible: (s) => can(s, 'extra-trip.import', OWNER) }],
  },
  {
    resource: serviceTypes, route: 'service-types', titleKey: 'serviceTypes', summaryKey: 'serviceTypesSummary', newKey: 'newServiceType', editKey: 'newServiceType', icon: Tags,
    fields: [text('code', 'code', true), text('name', 'name', true), text('category', 'category'), { name: 'active', labelKey: 'activeFlag', type: 'checkbox', defaultValue: true }],
    columns: [{ key: 'code', labelKey: 'code' }, { key: 'name', labelKey: 'name' }, { key: 'category', labelKey: 'category' }, { key: 'active', labelKey: 'activeFlag', kind: 'bool' }],
    read: (s) => can(s, 'service-type.read', OFFICE), create: (s) => can(s, 'service-type.create', OWNER),
  },
  {
    resource: pricedServices, route: 'priced-services', titleKey: 'pricedServices', summaryKey: 'pricedServicesSummary', newKey: 'newPricedService', editKey: 'newPricedService', icon: ReceiptText,
    fields: [text('name', 'name', true), text('category', 'category'), { name: 'defaultValueCents', labelKey: 'defaultValue', type: 'money', min: 0 }, text('observation', 'observation')],
    columns: [{ key: 'name', labelKey: 'name' }, { key: 'category', labelKey: 'category' }, { key: 'defaultValueCents', labelKey: 'defaultValue', kind: 'money' }],
    read: (s) => can(s, 'priced-service.read', OFFICE), create: (s) => can(s, 'priced-service.create', OWNER),
  },
  {
    resource: parts, route: 'parts', detail: true, titleKey: 'parts', summaryKey: 'partsSummary', newKey: 'newPart', editKey: 'newPart', icon: Package,
    fields: [text('name', 'name', true), text('unit', 'unit', true), text('category', 'category'), text('application', 'application'), num('minimumStock', 'minimumStock', true, 0),
      { name: 'unitValueCents', labelKey: 'unitValue', type: 'money', min: 0 }, text('defaultSupplier', 'supplier'), text('location', 'location'), text('observation', 'observation')],
    columns: [{ key: 'code', labelKey: 'code' }, { key: 'name', labelKey: 'name' }, { key: 'currentStock', labelKey: 'currentStock', kind: 'number' }, { key: 'minimumStock', labelKey: 'minimumStock', kind: 'number' }, { key: 'location', labelKey: 'location' }],
    read: (s) => can(s, 'part.read', OFFICE), create: (s) => can(s, 'part.create', OWNER_MECHANIC),
  },
  {
    resource: purchaseOrders, route: 'purchase-orders', detail: true, titleKey: 'purchaseOrders', summaryKey: 'purchaseOrdersSummary', newKey: 'newPurchaseOrder', editKey: 'newPurchaseOrder', icon: ShoppingCart,
    fields: [{ name: 'partUuid', labelKey: 'part', type: 'lookup', lookup: 'part', required: true }, num('quantity', 'quantity', true, 0.0001), text('suggestedSupplier', 'supplier'), text('observation', 'observation'),
      { name: 'vehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle' }],
    columns: [{ key: 'number', labelKey: 'number' }, { key: 'quantity', labelKey: 'quantity', kind: 'number' }, { key: 'suggestedSupplier', labelKey: 'supplier' }, { key: 'expectedDeliveryDate', labelKey: 'expectedDelivery', kind: 'date' }, { key: 'status', labelKey: 'status', kind: 'badge', prefix: 'poStatus' }],
    read: (s) => can(s, 'purchase-order.read', OFFICE), create: (s) => can(s, 'purchase-order.create', OWNER_MECHANIC),
  },
  {
    resource: vehicleExpenses, route: 'vehicle-expenses', titleKey: 'vehicleExpenses', summaryKey: 'vehicleExpensesSummary', newKey: 'newExpense', editKey: 'newExpense', icon: Warehouse,
    fields: [{ name: 'vehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle', required: true }, text('category', 'category', true), text('competencePeriod', 'competencePeriod', true), date('issueDate', 'issueDate', true),
      text('supplier', 'supplier'), text('invoiceNumber', 'invoice'), text('description', 'description'), { name: 'valueCents', labelKey: 'amount', type: 'money', required: true, min: 0.01 }],
    columns: [{ key: 'category', labelKey: 'category' }, { key: 'competencePeriod', labelKey: 'competencePeriod' }, { key: 'issueDate', labelKey: 'issueDate', kind: 'date' }, { key: 'supplier', labelKey: 'supplier' }, { key: 'valueCents', labelKey: 'amount', kind: 'money' }],
    read: (s) => can(s, 'vehicle-expense.read', OFFICE), create: (s) => can(s, 'vehicle-expense.create', OWNER),
  },
  {
    resource: garageServices, route: 'garage-services', titleKey: 'garageServices', summaryKey: 'garageServicesSummary', newKey: 'newGarageService', editKey: 'editGarageService', icon: Wrench,
    fields: [text('name', 'name', true), num('displayOrder', 'displayOrder', true, 0), pick('serviceGroup', 'serviceGroup', ['External', 'Internal'], 'serviceGroup', true),
      { name: 'requiredForDeparture', labelKey: 'requiredForDeparture', type: 'checkbox' }, { name: 'governedByTank', labelKey: 'governedByTank', type: 'checkbox' }, { name: 'active', labelKey: 'activeFlag', type: 'checkbox', defaultValue: true }],
    columns: [{ key: 'displayOrder', labelKey: 'displayOrder', kind: 'number' }, { key: 'name', labelKey: 'name' }, { key: 'serviceGroup', labelKey: 'serviceGroup', kind: 'badge', prefix: 'serviceGroup' }, { key: 'requiredForDeparture', labelKey: 'requiredForDeparture', kind: 'bool' }, { key: 'active', labelKey: 'activeFlag', kind: 'bool' }],
    read: (s) => can(s, 'garage.service-catalogue.read', OFFICE), create: (s) => can(s, 'garage.service-catalogue.create', OWNER), edit: (s) => can(s, 'garage.service-catalogue.update', OWNER),
  },
  {
    resource: checklistTemplates, route: 'checklist-templates', detail: true, titleKey: 'checklistTemplates', summaryKey: 'checklistTemplatesSummary', newKey: 'newChecklistTemplate', editKey: 'newChecklistTemplate', icon: ClipboardCheck,
    fields: [text('name', 'name', true), pick('checklistType', 'checklistType', ['Departure', 'Return', 'Standalone'], 'checklistType', true), { name: 'active', labelKey: 'activeFlag', type: 'checkbox', defaultValue: true },
      { name: 'items', labelKey: 'items', type: 'list', required: true, itemFields: [text('description', 'description', true), { name: 'generatesWorkOrder', labelKey: 'generatesWorkOrder', type: 'checkbox' }] }],
    columns: [{ key: 'name', labelKey: 'name' }, { key: 'checklistType', labelKey: 'checklistType', kind: 'badge', prefix: 'checklistType' }, { key: 'active', labelKey: 'activeFlag', kind: 'bool' }],
    read: (s) => can(s, 'checklist-template.read', OFFICE), create: (s) => can(s, 'checklist-template.create', OWNER),
  },
  {
    resource: inspectionModels, route: 'inspection-models', titleKey: 'inspectionModels', summaryKey: 'inspectionModelsSummary', newKey: 'newInspectionModel', editKey: 'newInspectionModel', icon: SearchCheck,
    fields: [text('name', 'name', true), { name: 'generatesWorkOrder', labelKey: 'generatesWorkOrder', type: 'checkbox' }, num('periodicityDays', 'periodicityDays', false, 1), text('observation', 'observation'),
      { name: 'active', labelKey: 'activeFlag', type: 'checkbox', defaultValue: true }, { name: 'items', labelKey: 'inspectionItems', type: 'list', required: true, scalar: true, itemFields: [text('value', 'description', true)] }],
    columns: [{ key: 'name', labelKey: 'name' }, { key: 'generatesWorkOrder', labelKey: 'generatesWorkOrder', kind: 'bool' }, { key: 'periodicityDays', labelKey: 'periodicityDays', kind: 'number' }, { key: 'active', labelKey: 'activeFlag', kind: 'bool' }],
    read: (s) => can(s, 'inspection-model.read', OFFICE), create: (s) => can(s, 'inspection-model.create', OWNER_MECHANIC),
    shortcuts: [{ to: '/inspections/new', labelKey: 'recordInspection', visible: (s) => can(s, 'technical-inspection.create', OWNER_MECHANIC) }],
  },
  {
    resource: maintenanceWindows, route: 'maintenance-windows', titleKey: 'maintenanceWindows', summaryKey: 'maintenanceWindowsSummary', newKey: 'newMaintenanceWindow', editKey: 'newMaintenanceWindow', icon: CalendarClock,
    fields: [{ name: 'vehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle', required: true }, date('date', 'date', true), { name: 'plannedStart', labelKey: 'plannedStart', type: 'datetime' }, { name: 'plannedEnd', labelKey: 'plannedEnd', type: 'datetime' },
      { name: 'affectsSchedule', labelKey: 'affectsSchedule', type: 'checkbox' }, text('origin', 'origin'),
      { name: 'workOrderUuids', labelKey: 'workOrders', type: 'list', scalar: true, itemFields: [{ name: 'value', labelKey: 'workOrders', type: 'lookup', lookup: 'workOrder', required: true }] }],
    columns: [{ key: 'date', labelKey: 'date', kind: 'date' }, { key: 'plannedStart', labelKey: 'plannedStart', kind: 'datetime' }, { key: 'affectsSchedule', labelKey: 'affectsSchedule', kind: 'bool' }, { key: 'status', labelKey: 'status', kind: 'badge', prefix: 'windowStatus' }],
    read: (s) => can(s, 'maintenance-plan.read', OFFICE), create: (s) => can(s, 'maintenance-plan.create', OWNER_MECHANIC),
  },
  {
    resource: preventivePlans, route: 'preventive-plans', detail: true, titleKey: 'preventivePlans', summaryKey: 'preventivePlansSummary', newKey: 'newPreventivePlan', editKey: 'editPreventivePlan', icon: ListChecks,
    fields: [{ name: 'vehicleUuid', labelKey: 'vehicle', type: 'lookup', lookup: 'vehicle', required: true, createOnly: true }, { ...text('planName', 'planName', true), createOnly: true },
      pick('controlType', 'controlType', ['Kilometers', 'Days', 'Both'], 'controlType', true), num('intervalKm', 'intervalKm', false, 0.0001), num('intervalDays', 'intervalDays', false, 1), num('lastServiceKm', 'lastServiceKm', false, 0), date('lastServiceDate', 'lastServiceDate')],
    columns: [{ key: 'planName', labelKey: 'planName' }, { key: 'controlType', labelKey: 'controlType', kind: 'badge', prefix: 'controlType' }, { key: 'extensionLimitKm', labelKey: 'extensionLimitKm', kind: 'number' }, { key: 'status', labelKey: 'status', kind: 'badge', prefix: 'planStatus' }],
    read: (s) => can(s, 'preventive-plan.read', OFFICE), create: (s) => can(s, 'preventive-plan.create', OWNER_MECHANIC), edit: (s) => can(s, 'preventive-plan.update', OWNER_MECHANIC),
  },
]
