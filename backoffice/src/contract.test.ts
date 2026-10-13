import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, it } from 'vitest'
import { resourceConfigs } from './features/resources'
import { entryFields, gaugeSettingFields, receiptFields, ruleFields, tankFields, unifyFields } from './pages/FuelPages'
import type { FieldDef } from './features/fields'

// Os testes de tela simulam a API, e uma simulação só devolve o que o autor lembrou de pôr nela. Este teste lê o
// próprio JSON do backend: todo campo obrigatório da requisição precisa existir no formulário (ou ser derivado pelo servidor).
const json = join(__dirname, '../../backend/application/src/endpoints/json')
const camel = (name: string) => name.replace(/_([a-z0-9])/g, (_, c: string) => c.toUpperCase())

function required(file: string, struct: string): string[] {
  const source = readFileSync(join(json, file), 'utf8')
  const body = new RegExp(`pub struct ${struct}\\s*\\{([\\s\\S]*?)\\n\\}`).exec(source)?.[1] ?? ''
  const names: string[] = []
  let attributes = ''
  for (const line of body.split('\n')) {
    const trimmed = line.trim()
    if (trimmed.startsWith('#[')) { attributes += trimmed; continue }
    const field = /^pub (\w+):\s*(.+?),?$/.exec(trimmed)
    if (field) {
      const optional = field[2].startsWith('Option<') || /serde\(([^)]*\b)?default/.test(attributes) || /skip_deserializing/.test(attributes)
      if (!optional) names.push(camel(field[1]))
    }
    if (!trimmed.startsWith('///')) attributes = ''
  }
  return names
}
const names = (fields: FieldDef[]) => fields.flatMap((f) => (f.type === 'list' ? [f.name] : [f.name]))
const covered = (file: string, struct: string, fields: FieldDef[], derived: string[] = []) => required(file, struct).filter((n) => !names(fields).includes(n) && !derived.includes(n))
const fieldsOf = (route: string) => resourceConfigs.find((c) => c.route === route)!.fields

describe('forms cover what the API requires', () => {
  const cases: [string, string, string, FieldDef[], string[]?][] = [
    ['vehicles', 'vehicle_json.rs', 'VehicleJson', fieldsOf('vehicles')], ['customers', 'customer_json.rs', 'CustomerJson', fieldsOf('customers')],
    ['holidays', 'holiday_json.rs', 'HolidayJson', fieldsOf('holidays')], ['transport demands', 'transport_demand_json.rs', 'TransportDemandJson', fieldsOf('transport-demands')],
    ['extra trips', 'extra_trip_json.rs', 'ExtraTripJson', fieldsOf('extra-trips')], ['service types', 'service_type_json.rs', 'ServiceTypeJson', fieldsOf('service-types')],
    ['priced services', 'priced_service_json.rs', 'PricedServiceJson', fieldsOf('priced-services')], ['parts', 'part_json.rs', 'PartJson', fieldsOf('parts')],
    ['purchase orders', 'purchase_order_json.rs', 'PurchaseOrderJson', fieldsOf('purchase-orders')], ['vehicle expenses', 'vehicle_expense_json.rs', 'VehicleExpenseJson', fieldsOf('vehicle-expenses')],
    ['garage services', 'garage_service_model_json.rs', 'GarageServiceModelJson', fieldsOf('garage-services')], ['checklist templates', 'checklist_template_json.rs', 'ChecklistTemplateJson', fieldsOf('checklist-templates')],
    ['inspection models', 'inspection_model_json.rs', 'InspectionModelJson', fieldsOf('inspection-models')], ['maintenance windows', 'maintenance_plan_json.rs', 'MaintenancePlanJson', fieldsOf('maintenance-windows')],
    ['preventive plans', 'preventive_plan_json.rs', 'PreventivePlanJson', fieldsOf('preventive-plans')], ['fuel entries', 'fuel_entry_json.rs', 'FuelEntryJson', entryFields],
    ['fuel receipts', 'fuel_entry_json.rs', 'FuelReceiptJson', receiptFields], ['fuel unify', 'fuel_entry_json.rs', 'FuelUnifyJson', unifyFields],
    ['internal tank', 'internal_tank_json.rs', 'InternalTankJson', tankFields], ['gauge settings', 'fuel_gauge_setting_json.rs', 'FuelGaugeSettingJson', gaugeSettingFields],
    ['tenant rules', 'tenant_rule_setting_json.rs', 'TenantRuleSettingJson', ruleFields],
  ]
  it.each(cases)('%s', (_name, file, struct, fields, derived) => {
    expect(required(file, struct).length, `${struct} was not found or has no required field`).toBeGreaterThan(0)
    expect(covered(file, struct, fields, derived)).toEqual([])
  })
})
