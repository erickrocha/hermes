import { waitFor } from '@testing-library/react'
import type { ComponentType } from 'react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api'
import { renderPage, signIn } from '../test/render'
import { LoginPage, PasswordPage } from './AuthPages'
import { ChecklistRunNewPage, ChecklistTemplateDetailPage, TechnicalInspectionNewPage } from './ChecklistPages'
import { DashboardPage } from './DashboardPage'
import { CustomerDayOffPage, CustomerDetailPage, DemandDetailPage, VehicleAssignPage, VehicleDetailPage, VehicleOdometerPage } from './FleetPages'
import { GarageCallCancelPage, GarageCallPage, GarageMonitorPage, GarageOpenPage, GaragePage, GarageTriagePage } from './GaragePages'
import { PreventiveAlertPage, PreventiveExtensionPage, PreventivePlanDetailPage } from './PreventivePages'
import { SchedulePage, TripImportPage } from './SchedulePages'
import { SettingsPage } from './SettingsPage'
import { PartAdjustPage, PartDetailPage, PartEntryPage, PurchaseOrderDetailPage, PurchaseOrderOrderedPage } from './StockPages'

// Cada tela monta sob o store real, com leituras vazias, sem quebrar o render.
const screens: [string, ComponentType][] = [
  ['LoginPage', LoginPage], ['PasswordPage', PasswordPage], ['DashboardPage', DashboardPage], ['SettingsPage', SettingsPage],
  ['ChecklistTemplateDetailPage', ChecklistTemplateDetailPage], ['ChecklistRunNewPage', ChecklistRunNewPage], ['TechnicalInspectionNewPage', TechnicalInspectionNewPage],
  ['VehicleDetailPage', VehicleDetailPage], ['VehicleOdometerPage', VehicleOdometerPage], ['VehicleAssignPage', VehicleAssignPage],
  ['CustomerDetailPage', CustomerDetailPage], ['CustomerDayOffPage', CustomerDayOffPage], ['DemandDetailPage', DemandDetailPage],
  ['GaragePage', GaragePage], ['GarageOpenPage', GarageOpenPage], ['GarageCallPage', GarageCallPage], ['GarageCallCancelPage', GarageCallCancelPage],
  ['GarageTriagePage', GarageTriagePage], ['GarageMonitorPage', GarageMonitorPage],
  ['PreventivePlanDetailPage', PreventivePlanDetailPage], ['PreventiveExtensionPage', PreventiveExtensionPage], ['PreventiveAlertPage', PreventiveAlertPage],
  ['SchedulePage', SchedulePage], ['TripImportPage', TripImportPage],
  ['PartDetailPage', PartDetailPage], ['PartEntryPage', PartEntryPage], ['PartAdjustPage', PartAdjustPage],
  ['PurchaseOrderDetailPage', PurchaseOrderDetailPage], ['PurchaseOrderOrderedPage', PurchaseOrderOrderedPage],
]

describe('page smoke', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.spyOn(console, 'error').mockImplementation(() => {})
    vi.spyOn(api, 'get').mockResolvedValue({ data: { items: [], page: 0, pageSize: 25, totalItems: 0, totalPages: 0 } } as never)
    signIn('TenantOwner')
  })

  it.each(screens)('%s renders without crashing', async (_name, Page) => {
    const { container } = renderPage(<Page />, '/x/u1', '/x/:uuid')
    await waitFor(() => expect(container.innerHTML).not.toBe(''))
  })
})
