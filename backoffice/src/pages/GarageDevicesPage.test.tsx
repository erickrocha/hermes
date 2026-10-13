import { fireEvent, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api'
import { customRoutes } from '../routes'
import { renderPage, session, signIn } from '../test/render'
import { GarageDevicesPage } from './GarageDevicesPage'

const device = { uuid: 'device-1', name: 'North wall', paired: true, revoked: false, pairedAt: '2026-10-13T12:00:00Z', revokedAt: null }

describe('GarageDevicesPage (BO-08)', () => {
  beforeEach(() => { vi.restoreAllMocks() })

  it('protects device management with the tenant-owner role', () => {
    const route = customRoutes.find((item) => item.path === '/garage/devices')
    expect(route?.allow(session('TenantOwner'))).toBe(true)
    expect(route?.allow(session('Mechanic'))).toBe(false)
    expect(route?.allow(session('TenantUser'))).toBe(false)
  })

  it('issues a one-time pairing code and displays it to the tenant owner', async () => {
    vi.spyOn(api, 'get').mockResolvedValue({ data: [] } as never)
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: { deviceUuid: 'device-1', deviceName: 'North wall', code: 'f842a79a', expiresAt: '2026-10-13T12:10:00Z' } } as never)
    signIn('TenantOwner')
    renderPage(<GarageDevicesPage />, '/garage/devices', '/garage/devices')

    fireEvent.change(await screen.findByLabelText(/device name|nome do dispositivo/i), { target: { value: 'North wall' } })
    fireEvent.click(screen.getByRole('button', { name: /issue pairing code|gerar código de pareamento/i }))

    expect(await screen.findByLabelText(/pairing code|código de pareamento/i)).toHaveTextContent('f842a79a')
    expect(post).toHaveBeenCalledWith('/garage-device/pairing-code', { name: 'North wall' })
  })

  it('requires confirmation before revoking a paired display credential', async () => {
    vi.spyOn(api, 'get').mockResolvedValue({ data: [device] } as never)
    const remove = vi.spyOn(api, 'delete').mockResolvedValue({ data: undefined } as never)
    signIn('TenantOwner')
    renderPage(<GarageDevicesPage />, '/garage/devices', '/garage/devices')

    expect(await screen.findByText('North wall')).toBeInTheDocument()
    fireEvent.click(screen.getByRole('button', { name: /revoke device credential|revogar credencial do dispositivo/i }))
    expect(remove).not.toHaveBeenCalled()
    fireEvent.click(screen.getByRole('button', { name: /confirm|confirmar/i }))

    await waitFor(() => expect(remove).toHaveBeenCalledWith('/garage-device/uuid/device-1'))
  })
})