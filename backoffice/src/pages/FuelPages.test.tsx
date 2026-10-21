import { fireEvent, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api'
import { renderPage, signIn } from '../test/render'
import { FuelEntryNewPage, FuelGaugePage, FuelPage, FuelReceiptPage, FuelReportPage, FuelRestorePage, InternalTankConfigPage, InternalTankPage, RuleSettingsPage } from './FuelPages'

const page = (items: unknown[]) => ({ data: { items, page: 0, pageSize: 25, totalItems: items.length, totalPages: 1 } }) as never
const notFound = () => Promise.reject(Object.assign(new Error('nf'), { isAxiosError: true, response: { status: 404, data: { message: 'none' } } }))
const save = /^save$|^salvar$|^guardar$/i
const vehicle = { uuid: 'v1', plate: 'ABC1D23', model: 'Bus' }
const entry = { uuid: 'f1', vehicleUuid: 'v1', recordedAt: '2026-10-01T08:00:00', volumeLiters: 80, valueCents: 48000, fullTank: true, origin: 'Manual' }

describe('fuel (C-029, HRMS-942…955)', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => {
      if (url === '/vehicle') return page([vehicle])
      if (url === '/user') return page([])
      if (url === '/fuel-entry') return page([entry])
      return page([])
    })
  })

  it('lists the fuellings with the vehicle plate; the mechanic gets the tools, the tenant user only the readers', async () => {
    signIn('Mechanic')
    const { unmount } = renderPage(<FuelPage />)
    expect(await screen.findByText(/ABC1D23/)).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /new fuelling|novo abastecimento|nuevo abastecimiento/i })).toBeInTheDocument()
    expect(screen.getByRole('button', { name: /sync with the provider|sincronizar com o provedor|sincronizar con el proveedor/i })).toBeInTheDocument()
    unmount()
    signIn('TenantUser')
    renderPage(<FuelPage />)
    await screen.findByText(/ABC1D23/)
    expect(screen.queryByRole('button', { name: /sync with the provider|sincronizar com o provedor|sincronizar con el proveedor/i })).toBeNull()
    expect(screen.getByRole('link', { name: /fuelling report|relatório de abastecimentos|informe de abastecimientos/i })).toBeInTheDocument()
  })

  it('syncs with the provider only after confirming and reports the outcome', async () => {
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: { fetched: 3, imported: 2, updated: 1, reconciled: 0, unmatched: 0 } } as never)
    signIn('Mechanic')
    renderPage(<FuelPage />)
    fireEvent.click(await screen.findByRole('button', { name: /sync with the provider|sincronizar com o provedor|sincronizar con el proveedor/i }))
    expect(post).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: /confirm|confirmar/i }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/fuel-entry/sync', undefined))
    expect(await screen.findByRole('status')).toHaveTextContent(/3/)
  })

  it('posts a new fuelling with money in cents and the full-tank flag', async () => {
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    signIn('Mechanic')
    renderPage(<FuelEntryNewPage />, '/fuel/new', '/fuel/new')
    fireEvent.change(await screen.findByLabelText(/^vehicle|^veículo|^vehículo/i), { target: { value: 'v1' } })
    fireEvent.change(screen.getByLabelText(/^recorded|^registrada/i), { target: { value: '2026-10-01T08:00' } })
    fireEvent.change(screen.getByLabelText(/^volume/i), { target: { value: '80.5' } })
    fireEvent.change(screen.getByLabelText(/^amount|^valor|^monto/i), { target: { value: '483.00' } })
    fireEvent.click(screen.getByLabelText(/full tank|tanque cheio|tanque lleno/i))
    fireEvent.click(screen.getByRole('button', { name: save }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/fuel-entry', { vehicleUuid: 'v1', recordedAt: '2026-10-01T08:00:00', volumeLiters: 80.5, valueCents: 48300, fullTank: true }))
  })

  it('lets the driver register a receipt but not create a fuelling', async () => {
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    signIn('Driver')
    renderPage(<FuelReceiptPage />, '/fuel/receipt', '/fuel/receipt')
    fireEvent.change(await screen.findByLabelText(/^vehicle|^veículo|^vehículo/i), { target: { value: 'v1' } })
    fireEvent.change(screen.getByLabelText(/^prefix|^prefixo|^prefijo/i), { target: { value: 'P7' } })
    fireEvent.change(screen.getByLabelText(/^recorded|^registrada/i), { target: { value: '2026-10-01T08:00' } })
    fireEvent.change(screen.getByLabelText(/^volume/i), { target: { value: '60' } })
    fireEvent.change(screen.getByLabelText(/^amount|^valor|^monto/i), { target: { value: '300' } })
    fireEvent.click(screen.getByRole('button', { name: save }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/fuel-entry/receipt', expect.objectContaining({ vehicleUuid: 'v1', vehiclePrefix: 'P7', volumeLiters: 60, valueCents: 30000, fullTank: false })))
  })

  it('restores the latest deleted fuelling through a query on the vehicle', async () => {
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    signIn('Mechanic')
    renderPage(<FuelRestorePage />, '/fuel/restore', '/fuel/restore')
    fireEvent.change(await screen.findByLabelText(/^vehicle|^veículo|^vehículo/i), { target: { value: 'v1' } })
    fireEvent.click(screen.getByRole('button', { name: save }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/fuel-entry/restore?vehicleUuid=v1', undefined))
  })

  it('shows the report totals for the chosen filters', async () => {
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => {
      if (url === '/fuel-entry/report') return { data: { entries: [{ ...entry, station: 'Posto Central' }], totalLiters: 80, totalValueCents: 48000 } } as never
      return page([vehicle])
    })
    signIn('TenantUser')
    renderPage(<FuelReportPage />)
    fireEvent.click(await screen.findByRole('button', { name: /generate|gerar|generar/i }))
    expect(await screen.findByText('Posto Central')).toBeInTheDocument()
    expect(screen.getAllByText(/480[.,]00/).length).toBeGreaterThan(0)
  })

  it('shows the gauge and the average for a vehicle, or the reason when it is unavailable', async () => {
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => {
      if (url === '/fuel-entry/gauge') return { data: { available: true, percent: 75, remainingLiters: 300, rangeKm: 3000, estimated: false } } as never
      if (url === '/fuel-entry/average') return { data: { kmPerLiter: 10.4, segments: 3, mature: true, effectiveKmPerLiter: 10.4, effectiveSource: 'Own' } } as never
      return page([vehicle])
    })
    signIn('TenantUser')
    renderPage(<FuelGaugePage />)
    fireEvent.change(await screen.findByLabelText(/^vehicle|^veículo|^vehículo/i), { target: { value: 'v1' } })
    fireEvent.click(screen.getByRole('button', { name: /generate|gerar|generar/i }))
    expect(await screen.findByText('75 %')).toBeInTheDocument()
    expect(screen.getByText('3000 km')).toBeInTheDocument(); expect(screen.getAllByText('10.4').length).toBe(2)
  })

  it('says the tank is not configured, and lets the owner configure it', async () => {
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => (url.startsWith('/internal-tank') ? notFound() : page([])))
    signIn('TenantOwner')
    const { unmount } = renderPage(<InternalTankPage />)
    expect(await screen.findByText(/not configured yet|ainda não foi configurado|aún no está configurado/i)).toBeInTheDocument()
    expect(screen.getByRole('link', { name: /configure|configurar/i })).toBeInTheDocument()
    unmount()
    const put = vi.spyOn(api, 'put').mockResolvedValue({ data: {} } as never)
    renderPage(<InternalTankConfigPage />, '/fuel/tank/edit', '/fuel/tank/edit')
    fireEvent.change(await screen.findByLabelText(/^capacity|^capacidade|^capacidad/i), { target: { value: '10000' } })
    fireEvent.change(screen.getByLabelText(/^reference stock|^estoque de referência|^stock de referencia/i), { target: { value: '6000' } })
    fireEvent.change(screen.getByLabelText(/^reference instant|^instante de referência|^instante de referencia/i), { target: { value: '2026-10-01T08:00' } })
    fireEvent.change(screen.getByLabelText(/^alert level|^nível de alerta|^nivel de alerta/i), { target: { value: '2000' } })
    fireEvent.change(screen.getByLabelText(/^reserve level|^nível de reserva|^nivel de reserva/i), { target: { value: '800' } })
    fireEvent.click(screen.getByRole('button', { name: save }))
    await waitFor(() => expect(put).toHaveBeenCalledWith('/internal-tank', { capacityLiters: 10000, referenceStockLiters: 6000, referenceAt: '2026-10-01T08:00:00', alertLevelLiters: 2000, reserveLevelLiters: 800 }))
  })

  it('loads the tenant rules into the form before the owner saves them', async () => {
    vi.spyOn(api, 'get').mockResolvedValue({ data: { avgMaxSegmentKm: 4000, avgMinKmPerLiter: 0.8, avgMaxKmPerLiter: 15, avgRecentWindowDays: 30, avgFallbackSegments: 8, avgMedianMinSegments: 4, avgMedianTolerance: 0.4, avgMedianMinSurvivors: 2, avgMinSegmentsForOwnAverage: 3,
      avgPeerCapacityTolerance: 0.3, receiptDuplicateVolumeToleranceLiters: 0.5, receiptDuplicateWindowMinutes: 10, reconciliationWindowHours: 48, reconciliationVolumeToleranceLiters: 1, reconciliationVolumeToleranceRatio: 0.03,
      preventiveAttentionKmMargin: 1000, preventiveAttentionDaysMargin: 15, preventiveMaxExtensionKm: 50000, avgPeerMinCount: 2, garageValidityHours: 36, garageMinTripAbsenceMinutes: 60,
      garageFuelPendingBelowPercent: 50, garageTripFuelExemptPercent: 75, garageAlertLowPercent: 40, garageAlertTripPercent: 80, garageMonitorUrgentMinutes: 60, garageMonitorTripWindowMinutes: 120, garageMonitorLineWindowMinutes: 30,
      garageMonitorCardLimit: 8, garageMonitorMatrixRows: 12, garageMonitorMatrixColumns: 10, garageFuellingFreshnessHours: 12, garageUtcOffsetMinutes: -180,
      garageBaseLatitude: null, garageBaseLongitude: null, garageBaseRadiusMeters: 75, garageCycleFallbackDistanceKm: 2, garageOdometerMaxSingleJumpKm: 300, nonExecutionGraceMinutes: 60, nonExecutionMinDwellMinutes: 10 } } as never)
    const put = vi.spyOn(api, 'put').mockResolvedValue({ data: {} } as never)
    signIn('TenantOwner')
    renderPage(<RuleSettingsPage />, '/tenants/tenant-1/manage', '/tenants/:uuid/manage')
    expect(await screen.findByDisplayValue('4000')).toBeInTheDocument()
    fireEvent.change(screen.getByDisplayValue('3'), { target: { value: '2' } })
    fireEvent.change(screen.getByLabelText(/^tolerance before|^tolerância antes|^tolerancia antes/i), { target: { value: '90' } })
    fireEvent.change(screen.getByLabelText(/^minimum stay for a manual|^permanência mínima do registro|^permanencia mínima del registro/i), { target: { value: '15' } })
    fireEvent.click(screen.getByRole('button', { name: save }))
    await waitFor(() => expect(put).toHaveBeenCalledWith('/tenant-rule-settings', expect.objectContaining({ avgMinSegmentsForOwnAverage: 2, avgMaxSegmentKm: 4000, nonExecutionGraceMinutes: 90, nonExecutionMinDwellMinutes: 15 })))
  })
})
