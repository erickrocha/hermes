import { fireEvent, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api'
import { resourceConfigs } from '../features/resources'
import { renderPage, signIn } from '../test/render'
import { ResourceNewRoute } from '../components/organisms/ResourceRoutes'
import { ChecklistRunNewPage, ChecklistTemplateDetailPage, TechnicalInspectionNewPage } from './ChecklistPages'
import { GarageMonitorPage, GaragePage, GarageTriagePage } from './GaragePages'
import { PreventivePlanDetailPage } from './PreventivePages'
import { SchedulePage, TripImportPage, tripsFromCsv } from './SchedulePages'

const page = (items: unknown[]) => ({ data: { items, page: 0, pageSize: 50, totalItems: items.length, totalPages: 1 } }) as never
const save = /^save$|^salvar$|^guardar$/i
const config = (route: string) => resourceConfigs.find((c) => c.route === route)!
const vehicle = { uuid: 'v1', plate: 'ABC1D23', model: 'Bus' }

describe('operations screens', () => {
  beforeEach(() => { vi.restoreAllMocks() })

  it('creates a checklist template with its items from a repeating group', async () => {
    vi.spyOn(api, 'get').mockResolvedValue(page([]))
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    signIn('TenantOwner')
    renderPage(<ResourceNewRoute config={config('checklist-templates')} />, '/checklist-templates/new', '/checklist-templates/new')
    fireEvent.change(await screen.findByLabelText(/^name|^nome|^nombre/i), { target: { value: 'Pre-trip' } })
    fireEvent.change(screen.getByLabelText(/^type|^tipo/i), { target: { value: 'Departure' } })
    fireEvent.click(screen.getByRole('button', { name: save }))
    expect(await screen.findByText(/required field|campo obrigat/i)).toBeInTheDocument()
    expect(post).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: /add row|adicionar linha|agregar fila/i }))
    fireEvent.change(await screen.findByLabelText(/^description|^descrição|^descripción/i), { target: { value: 'Brakes' } })
    fireEvent.click(screen.getByLabelText(/generates work order|gera ordem|genera orden/i))
    fireEvent.click(screen.getByRole('button', { name: save }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/checklist-template', { name: 'Pre-trip', checklistType: 'Departure', active: true, items: [{ description: 'Brakes', generatesWorkOrder: true }] }))
  })

  it('builds the checklist run from the template: one answer per item, sent with the template and its type', async () => {
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => {
      if (url === '/checklist-template/uuid/t1') return { data: { uuid: 't1', name: 'Pre-trip', checklistType: 'Departure', items: [{ uuid: 'i1', description: 'Brakes' }, { uuid: 'i2', description: 'Horn' }] } } as never
      if (url === '/vehicle') return page([vehicle])
      if (url === '/user') return page([{ uuid: 'd1', name: 'Dora', profileKey: 'Driver', extraPermissions: [], permissions: ['driver.act', 'vehicle.read@own'] }])
      return page([])
    })
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    signIn('TenantOwner')
    renderPage(<ChecklistRunNewPage />, '/checklist-templates/t1/run', '/checklist-templates/:uuid/run')
    expect(await screen.findByDisplayValue('Brakes')).toBeDisabled()
    expect(screen.getByDisplayValue('Horn')).toBeInTheDocument()
    fireEvent.change(screen.getByLabelText(/^vehicle|^veículo|^vehículo/i), { target: { value: 'v1' } })
    fireEvent.change(screen.getByLabelText(/^driver|^motorista|^conductor/i), { target: { value: 'd1' } })
    fireEvent.change(screen.getByLabelText(/odometer|odômetro|odómetro/i), { target: { value: '1200' } })
    fireEvent.change(screen.getAllByLabelText(/^answer|^resposta|^respuesta/i)[0], { target: { value: 'NonConforming' } })
    fireEvent.click(screen.getByRole('button', { name: save }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/checklist-run', {
      vehicleUuid: 'v1', driverUuid: 'd1', odometerKm: 1200, checklistTemplateUuid: 't1', checklistType: 'Departure',
      answers: [{ checklistTemplateItemUuid: 'i1', status: 'NonConforming' }, { checklistTemplateItemUuid: 'i2', status: 'Conforming' }] }))
  })

  it('lets the owner make an active departure template the default, once', async () => {
    let isDefault = false
    vi.spyOn(api, 'get').mockImplementation(async () => ({ data: { uuid: 't1', name: 'Pre-trip', checklistType: 'Departure', active: true, isDefault, items: [{ uuid: 'i1', description: 'Brakes' }] } }) as never)
    const put = vi.spyOn(api, 'put').mockImplementation(async () => { isDefault = true; return { data: {} } as never })
    signIn('TenantOwner')
    renderPage(<ChecklistTemplateDetailPage />, '/checklist-templates/t1', '/checklist-templates/:uuid')
    fireEvent.click(await screen.findByRole('button', { name: /set as default|definir como padr|definir como predeterminado/i }))
    await waitFor(() => expect(put).toHaveBeenCalledWith('/checklist-template/uuid/t1/default', undefined))
    await waitFor(() => expect(screen.queryByRole('button', { name: /set as default|definir como padr|definir como predeterminado/i })).not.toBeInTheDocument())
  })

  it('offers no default action for a standalone template or to a non-owner', async () => {
    vi.spyOn(api, 'get').mockResolvedValue({ data: { uuid: 't1', name: 'One-off', checklistType: 'Standalone', active: true, isDefault: false, items: [] } } as never)
    signIn('TenantOwner')
    const { unmount } = renderPage(<ChecklistTemplateDetailPage />, '/checklist-templates/t1', '/checklist-templates/:uuid')
    await screen.findByText('One-off')
    expect(screen.queryByRole('button', { name: /set as default|definir como padr|definir como predeterminado/i })).not.toBeInTheDocument()
    unmount()
    vi.spyOn(api, 'get').mockResolvedValue({ data: { uuid: 't1', name: 'Pre-trip', checklistType: 'Departure', active: true, isDefault: false, items: [] } } as never)
    signIn('TenantUser')
    renderPage(<ChecklistTemplateDetailPage />, '/checklist-templates/t1', '/checklist-templates/:uuid')
    await screen.findByText('Pre-trip')
    expect(screen.queryByRole('button', { name: /set as default|definir como padr|definir como predeterminado/i })).not.toBeInTheDocument()
  })

  it('records an inspection with the model chosen by name', async () => {
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => (url === '/inspection-model' ? page([{ uuid: 'm1', name: 'Brake check' }]) : page([vehicle])))
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    signIn('Mechanic')
    renderPage(<TechnicalInspectionNewPage />, '/inspections/new', '/inspections/new')
    await screen.findByRole('option', { name: 'Brake check' })
    await screen.findByRole('option', { name: /ABC1D23/ })
    fireEvent.change(screen.getByLabelText(/^vehicle|^veículo|^vehículo/i), { target: { value: 'v1' } })
    fireEvent.change(screen.getByLabelText(/inspection model|modelo de inspeção|modelo de inspección/i), { target: { value: 'Brake check' } })
    fireEvent.click(screen.getByRole('button', { name: /add row|adicionar linha|agregar fila/i }))
    fireEvent.change(await screen.findByLabelText(/^description|^descrição|^descripción/i), { target: { value: 'Pads' } })
    fireEvent.click(screen.getByRole('button', { name: save }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/technical-inspection', { vehicleUuid: 'v1', inspectionModel: 'Brake check', items: [{ description: 'Pads', conforming: true }] }))
  })

  it('opens the work order of an overdue plan and goes to it', async () => {
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => (url === '/preventive-plan/uuid/p1' ? { data: { uuid: 'p1', planName: 'Oil', controlType: 'Kilometers', status: 'Overdue', intervalKm: 10000 } } as never : page([])))
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: { workOrderUuid: 'w9', number: 'OS-9', created: true } } as never)
    signIn('Mechanic')
    renderPage(<PreventivePlanDetailPage />, '/preventive-plans/p1', '/preventive-plans/:uuid')
    fireEvent.click(await screen.findByRole('button', { name: /open work order|abrir ordem|abrir orden/i }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/preventive-plan/uuid/p1/work-order', undefined))
    expect(await screen.findByText('elsewhere')).toBeInTheDocument()
  })

  it('hides the work-order button for a plan that is up to date', async () => {
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => (url === '/preventive-plan/uuid/p1' ? { data: { uuid: 'p1', planName: 'Oil', controlType: 'Days', status: 'Ok' } } as never : page([])))
    signIn('Mechanic')
    renderPage(<PreventivePlanDetailPage />, '/preventive-plans/p1', '/preventive-plans/:uuid')
    await screen.findByText(/up to date|em dia|al día/i)
    expect(screen.queryByRole('button', { name: /open work order|abrir ordem|abrir orden/i })).toBeNull()
  })

  it('shows the yard queue and links a triaged vehicle to its triage', async () => {
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => (url === '/garage-attendance/queue' ? { data: [{ attendanceUuid: 'a1', vehicleUuid: 'v1', prefix: 'P7', requiredPending: ['Wash'], fillTankAlert: true, calledByManager: false }] } as never : page([vehicle])))
    signIn('Mechanic')
    renderPage(<GaragePage />)
    expect(await screen.findByRole('link', { name: 'P7' })).toHaveAttribute('href', '/garage/triage/a1')
    expect(screen.getByText('Wash')).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /open triage|abrir triagem|abrir triaje/i })).toBeInTheDocument()
  })

  it('marks a service of an open triage, and checks out only after confirming', async () => {
    const triage = { uuid: 'a1', vehicleUuid: 'v1', status: 'Open', checkedInAt: '2026-10-01T08:00:00', services: [{ uuid: 's1', name: 'Wash', serviceGroup: 'Internal', state: 'Pending', effectiveState: 'Pending', requiredForDeparture: true }] }
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => (url === '/garage-attendance/uuid/a1' ? { data: triage } as never : page([vehicle])))
    const put = vi.spyOn(api, 'put').mockResolvedValue({ data: {} } as never)
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    signIn('Mechanic')
    renderPage(<GarageTriagePage />, '/garage/triage/a1', '/garage/triage/:uuid')
    fireEvent.click(await screen.findByRole('button', { name: /^done$|^feito$|^hecho$/i }))
    await waitFor(() => expect(put).toHaveBeenCalledWith('/garage-attendance/uuid/a1/service/s1', { state: 'Performed' }))
    fireEvent.click(screen.getByRole('button', { name: /check out|dar saída|dar salida/i }))
    expect(post).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: /confirm|confirmar/i }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/garage-attendance/uuid/a1/checkout', undefined))
  })

  it('releases a triage with a justification and links a selected pendency to awaiting parts', async () => {
    const triage = { uuid: 'a1', vehicleUuid: 'v1', status: 'Open', checkedInAt: '2026-10-01T08:00:00', services: [{ uuid: 's1', name: 'Wash', serviceGroup: 'Internal', state: 'Pending', effectiveState: 'Pending', requiredForDeparture: true }] }
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => (url === '/garage-attendance/uuid/a1' ? { data: triage } as never : page([vehicle])))
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    signIn('Mechanic')
    renderPage(<GarageTriagePage />, '/garage/triage/a1', '/garage/triage/:uuid')
    fireEvent.click(await screen.findByRole('button', { name: /release with pendencies|liberar com pendências|liberar con pendientes/i }))
    fireEvent.change(screen.getByLabelText(/justification|justificativa|justificación/i), { target: { value: 'Waiting for brake pads' } })
    fireEvent.change(screen.getByLabelText(/odometer, if no official reading exists|odômetro, se não houver leitura oficial|odómetro, si no hay una lectura oficial/i), { target: { value: '1234' } })
    fireEvent.click(screen.getByLabelText('Wash'))
    fireEvent.click(screen.getByRole('button', { name: /record release|registrar liberação|registrar liberación/i }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/garage-attendance/uuid/a1/release-with-pendency', {
      justification: 'Waiting for brake pads', odometerKm: 1234, awaitingPartsServiceUuids: ['s1'],
    }))
  })

  it('shows the monitor sections, including the empty and error states', async () => {
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => {
      if (url === '/garage-attendance/monitor') return { data: [{ entry: { vehicleUuid: 'v1', prefix: 'P7', nextDeparture: '2026-10-01T10:00:00' }, readiness: 'Alert', urgent: true }] } as never
      if (url === '/garage-attendance/monitor/cards') return { data: { cards: [{ vehicleUuid: 'v1', prefix: 'P7', tankPercent: 30, needsRefuel: true }], notShown: 2 } } as never
      if (url === '/garage-attendance/call-list') return Promise.reject(new Error('down'))
      if (url === '/garage-attendance/monitor/matrix') return { data: { columns: [{ serviceModelUuid: 'm1', name: 'Wash' }], rows: [{ vehicleUuid: 'v1', prefix: 'P7', cells: [{ effectiveState: 'Performed' }] }], rowsNotShown: 0 } } as never
      return page([vehicle])
    })
    signIn('TenantUser')
    renderPage(<GarageMonitorPage />)
    expect(await screen.findByText(/30 %/)).toBeInTheDocument()
    expect(screen.getByText(/urgent|urgente/i)).toBeInTheDocument()
    expect(screen.getByText(/needs refuelling|precisa abastecer|necesita reabastecer/i)).toBeInTheDocument()
    expect(await screen.findByText('Wash')).toBeInTheDocument()
    expect(screen.getByText(/done|feito|hecho/i)).toBeInTheDocument()
    expect(await screen.findByRole('status')).toBeInTheDocument()
  })

  it('shows the effective schedule of the day with the driver and vehicle names and the crew alerts', async () => {
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => {
      if (url === '/schedule/effective') return { data: { date: '2026-10-05', items: [{ kind: 'Line', demandUuid: 'd1', name: 'Line 7', start: '2026-10-05T07:00:00', end: '2026-10-05T09:00:00', vehicleUuid: 'v1', resolvedFrom: 'Allocation', displacedByTrip: false, alerts: ['MissingDriver'] }], unclassifiedDemands: 1, malformedDemands: 0 } } as never
      if (url === '/vehicle') return page([vehicle])
      return page([])
    })
    signIn('TenantUser')
    renderPage(<SchedulePage />)
    expect(await screen.findByText('Line 7')).toBeInTheDocument()
    expect(await screen.findByText(/ABC1D23/)).toBeInTheDocument()
    expect(screen.getByText(/no driver|sem motorista|sin conductor/i)).toBeInTheDocument()
    expect(screen.getByRole('status')).toHaveTextContent(/1/)
  })

  it('reads trips from a CSV with the API field names, drops blanks and previews before importing', async () => {
    expect(tripsFromCsv('orderCode,tripDate,freightValueCents,destination\nA1,2026-10-05,150000,Campinas\nA2,2026-10-06,,\n')).toEqual([
      { orderCode: 'A1', tripDate: '2026-10-05', freightValueCents: 150000, destination: 'Campinas' }, { orderCode: 'A2', tripDate: '2026-10-06' }])
    const post = vi.spyOn(api, 'post').mockImplementation(async (url: string) => (url.endsWith('/preview')
      ? { data: { created: 1, updated: 1, rows: [{ row: 1, orderCode: 'A1', tripDate: '2026-10-05', action: 'Create' }, { row: 2, orderCode: 'A2', tripDate: '2026-10-06', action: 'Update' }] } } as never
      : { data: { created: 1, updated: 1 } } as never))
    signIn('TenantOwner')
    renderPage(<TripImportPage />, '/extra-trips/import', '/extra-trips/import')
    const file = new File(['orderCode,tripDate\nA1,2026-10-05\nA2,2026-10-06\n'], 'trips.csv', { type: 'text/csv' })
    fireEvent.change(await screen.findByLabelText(/csv file|arquivo csv|archivo csv/i), { target: { files: [file] } })
    expect(await screen.findByText(/would create 1 trips|criaria 1 viagens|crearía 1 viajes/i)).toBeInTheDocument()
    expect(post).toHaveBeenCalledTimes(1)
    fireEvent.click(screen.getByRole('button', { name: /^import$|^importar$/i }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/extra-trip/import', [{ orderCode: 'A1', tripDate: '2026-10-05' }, { orderCode: 'A2', tripDate: '2026-10-06' }]))
  })
})
