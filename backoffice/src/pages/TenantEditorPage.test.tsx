import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { Provider } from 'react-redux'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import '../i18n'
import { api } from '../api'
import { isValidCnpj, maskCnpj, maskPhone, maskPostalCode, TenantEditorPage } from './ManagementPages'
import { store } from '../store'

describe('TenantEditorPage country, province, and city selection', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  it('defaults to Brazil and reloads state options when switching to the United States', async () => {
    const apiGetSpy = vi.spyOn(api, 'get').mockImplementation(async (url: string) => {
      if (url === '/province') {
        return { data: [{ id: 1, acronym: 'SP', name: 'São Paulo', countryCode: 'BR' }] } as any
      }
      return { data: [] } as any
    })

    render(
      <Provider store={store}>
        <MemoryRouter initialEntries={['/tenants/new']}>
          <Routes>
            <Route path="/tenants/new" element={<TenantEditorPage />} />
          </Routes>
        </MemoryRouter>
      </Provider>
    )

    const brazil = screen.getByRole('radio', { name: /brazil|brasil/i }) as HTMLInputElement
    const unitedStates = screen.getByRole('radio', { name: /united states|estados unidos/i }) as HTMLInputElement
    expect(brazil.checked).toBe(true)
    expect(unitedStates.checked).toBe(false)

    await waitFor(() => {
      expect(apiGetSpy).toHaveBeenCalledWith('/province', expect.objectContaining({ params: expect.objectContaining({ countryCode: 'BR' }) }))
    })

    fireEvent.click(unitedStates)
    expect(unitedStates.checked).toBe(true)

    await waitFor(() => {
      expect(apiGetSpy).toHaveBeenCalledWith(
        '/province',
        expect.objectContaining({
          params: expect.objectContaining({ countryCode: 'US' }),
        })
      )
    })
  })
})

describe('Tenant form masks and CNPJ validation', () => {
  it('formats tax ID, postal code, and phone for Brazil and the United States', () => {
    expect(maskCnpj('11222333000181')).toBe('11.222.333/0001-81')
    expect(maskPostalCode('13000000', 'BR')).toBe('13000-000')
    expect(maskPostalCode('941051234', 'US')).toBe('94105-1234')
    expect(maskPhone('11987654321', 'BR')).toBe('(11) 98765-4321')
    expect(maskPhone('14155552671', 'US')).toBe('(415) 555-2671')
  })

  it('validates CNPJ check digits and rejects repeated digits', () => {
    expect(isValidCnpj('11.222.333/0001-81')).toBe(true)
    expect(isValidCnpj('11.222.333/0001-82')).toBe(false)
    expect(isValidCnpj('00.000.000/0000-00')).toBe(false)
  })

  it('shows the CNPJ mask and prevents submitting invalid check digits', async () => {
    vi.spyOn(api, 'get').mockResolvedValue({ data: [] } as never)
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)

    render(
      <Provider store={store}>
        <MemoryRouter initialEntries={['/tenants/new']}>
          <Routes>
            <Route path="/tenants/new" element={<TenantEditorPage />} />
          </Routes>
        </MemoryRouter>
      </Provider>
    )

    fireEvent.change(screen.getByLabelText(/legal name|nome empresarial/i), { target: { value: 'Empresa teste' } })
    fireEvent.change(screen.getByLabelText(/trading name|nome fantasia/i), { target: { value: 'Marca teste' } })
    const taxId = screen.getByLabelText(/CNPJ/i)
    fireEvent.change(taxId, { target: { value: '11222333000182' } })
    expect(taxId).toHaveValue('11.222.333/0001-82')
    fireEvent.click(screen.getByRole('button', { name: /save|salvar/i }))

    expect(await screen.findByRole('alert')).toHaveTextContent(/CNPJ/i)
    expect(post).not.toHaveBeenCalled()
  })
})
