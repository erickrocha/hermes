import { fireEvent, render, screen, within } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'
import '../../i18n'
import type { Permission } from '../../types'
import { PermissionPicker, grantFor } from './PermissionPicker'

const perm = (code: string, module: string, extra: Partial<Permission> = {}): Permission => ({ code, module, scope: 'Tenant', own: false, defaultFor: [], ...extra })
const catalogue: Permission[] = [perm('vehicle.read', 'Fleet', { own: true, defaultFor: ['TenantUser', 'Driver'] }), perm('vehicle.create', 'Fleet'), perm('work-order.read', 'Maintenance')]

// C-035: the picker emits grants -- `code`, or `code@own` once "own records only" is toggled on an own-capable permission.
describe('PermissionPicker', () => {
  it('renders one group per module with the default-for hint, and toggles a grant on and off', () => {
    const onChange = vi.fn()
    render(<PermissionPicker catalogue={catalogue} value={['work-order.read']} onChange={onChange} />)
    expect(screen.getAllByRole('tab').map((tab) => tab.textContent)).toEqual([expect.stringMatching(/Fleet|Frota/), expect.stringMatching(/Maintenance|Manutenção/)])
    expect(screen.getByText(/default for|padrão para/i)).toHaveTextContent(/Tenant user, Driver|Usuário, Motorista/)
    fireEvent.click(screen.getByRole('checkbox', { name: /^(Vehicle registration|Cadastro de veículo)$/ }))
    expect(onChange).toHaveBeenLastCalledWith(['work-order.read', 'vehicle.create'])
    fireEvent.click(screen.getByRole('tab', { name: /Maintenance|Manutenção/ }))
    fireEvent.click(screen.getByRole('checkbox', { name: /^(View work orders|Consulta de ordens de serviço)$/ }))
    expect(onChange).toHaveBeenLastCalledWith([])
  })

  it('offers the own-records toggle only for a held, own-capable permission, and emits code@own', () => {
    const onChange = vi.fn()
    const { rerender } = render(<PermissionPicker catalogue={catalogue} value={[]} onChange={onChange} />)
    expect(screen.queryByRole('checkbox', { name: /own/i })).not.toBeInTheDocument()
    rerender(<PermissionPicker catalogue={catalogue} value={['vehicle.read']} onChange={onChange} />)
    fireEvent.click(screen.getByRole('checkbox', { name: /vehicle\.read .*own/i }))
    expect(onChange).toHaveBeenLastCalledWith(['vehicle.read@own'])
    rerender(<PermissionPicker catalogue={catalogue} value={['vehicle.read@own']} onChange={onChange} />)
    expect(screen.getByRole('checkbox', { name: /^(View vehicles|Consulta de veículos)$/ })).toBeChecked()
    expect(screen.getByRole('checkbox', { name: /vehicle\.read .*own/i })).toBeChecked()
    fireEvent.click(screen.getByRole('checkbox', { name: /vehicle\.read .*own/i }))
    expect(onChange).toHaveBeenLastCalledWith(['vehicle.read'])
  })

  it('leaves excluded codes out and disables everything when read-only', () => {
    render(<PermissionPicker catalogue={catalogue} value={['vehicle.create']} onChange={() => {}} exclude={['work-order.read']} readOnly />)
    expect(screen.queryByText('work-order.read')).not.toBeInTheDocument()
    expect(screen.getByRole('checkbox', { name: /^(Vehicle registration|Cadastro de veículo)$/ })).toBeDisabled()
    expect(within(screen.getByTestId('permissions')).getAllByRole('checkbox')).toHaveLength(2)
  })

  it('says so when nothing is left to pick', () => {
    render(<PermissionPicker catalogue={catalogue} value={[]} onChange={() => {}} exclude={catalogue.map((p) => p.code)} />)
    expect(screen.getByText(/no permissions left|não há mais permissões/i)).toBeInTheDocument()
  })

  it('grantFor finds the grant held for a code in either form', () => {
    expect(grantFor(['vehicle.read@own'], 'vehicle.read')).toBe('vehicle.read@own')
    expect(grantFor(['vehicle.read'], 'vehicle.read')).toBe('vehicle.read')
    expect(grantFor([], 'vehicle.read')).toBeUndefined()
  })
})
