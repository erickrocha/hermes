import { fireEvent, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api'
import { renderPage, signIn } from '../test/render'
import { store } from '../store'
import { CustomerFormPage, fromCustomer, toCustomerPayload } from './CustomerPages'

const save = /^save$|^salvar$|^guardar$/i
const provinces = [{ id: 1, acronym: 'SP', name: 'São Paulo', countryCode: 'BR' }]
const failure = (status: number, errorKey: string, message: string) => Object.assign(new Error(errorKey), { isAxiosError: true, response: { status, data: { errorKey, message } } })
const tradeName = () => screen.getByLabelText(/trade name|nome fantasia|nombre de fantasía/i)
const taxId = () => screen.getByLabelText(/CPF \/ CNPJ/i)

describe('CustomerFormPage', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    store.dispatch({ type: 'data/currentTenant/rejected' })
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => {
      if (url === '/province') return { data: provinces } as never
      if (url === '/customer/uuid/c1') return { data: { uuid: 'c1', name: 'Acme', legalName: 'Acme Ltda', taxId: '11222333000181', status: 'Inactive', countryCode: 'BR', addressLine1: 'Rua A, 10', administrativeArea: 'SP', locality: 'Campinas', postalCode: '13000000' } } as never
      return { data: [] } as never
    })
    signIn('TenantOwner')
  })

  it('requires the trade name and does not post without it', async () => {
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    renderPage(<CustomerFormPage />, '/customers/new', '/customers/new')
    fireEvent.click(screen.getByRole('button', { name: save }))
    expect(await screen.findByText(/required field|campo obrigatório|campo obligatorio/i)).toBeInTheDocument()
    expect(post).not.toHaveBeenCalled()
  })

  it('rejects a CPF or CNPJ with wrong check digits, by the field', async () => {
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: {} } as never)
    renderPage(<CustomerFormPage />, '/customers/new', '/customers/new')
    fireEvent.change(tradeName(), { target: { value: 'Acme' } })
    fireEvent.change(taxId(), { target: { value: '52998224724' } })
    fireEvent.click(screen.getByRole('button', { name: save }))
    expect(await screen.findByText(/valid CPF or CNPJ|CPF ou CNPJ válido|CPF o CNPJ válido/i)).toBeInTheDocument()
    expect(post).not.toHaveBeenCalled()
    fireEvent.change(taxId(), { target: { value: '11222333000182' } })
    fireEvent.click(screen.getByRole('button', { name: save }))
    await waitFor(() => expect(post).not.toHaveBeenCalled())
    expect(screen.getByText(/valid CPF or CNPJ|CPF ou CNPJ válido|CPF o CNPJ válido/i)).toBeInTheDocument()
  })

  it('posts the customer in camelCase with the document digits-only and blanks left out, then returns to the list', async () => {
    const post = vi.spyOn(api, 'post').mockResolvedValue({ data: { uuid: 'c9' } } as never)
    renderPage(<CustomerFormPage />, '/customers/new', '/customers/new')
    fireEvent.change(tradeName(), { target: { value: ' Acme ' } })
    fireEvent.change(screen.getByLabelText(/legal name|razão social|razón social/i), { target: { value: 'Acme Ltda' } })
    fireEvent.change(taxId(), { target: { value: '529.982.247-25' } })
    expect(taxId()).toHaveValue('529.982.247-25')
    fireEvent.change(screen.getByLabelText(/^status/i), { target: { value: 'Inactive' } })
    fireEvent.change(screen.getByLabelText(/^address$|^endereço$|^dirección$/i), { target: { value: 'Rua A, 10' } })
    fireEvent.change(screen.getByLabelText(/postal code|^cep$|código postal/i), { target: { value: '13000000' } })
    fireEvent.click(screen.getByRole('button', { name: save }))
    await waitFor(() => expect(post).toHaveBeenCalledWith('/customer', { name: 'Acme', status: 'Inactive', legalName: 'Acme Ltda', taxId: '52998224725', addressLine1: 'Rua A, 10', postalCode: '13000000', countryCode: 'BR' }))
    expect(await screen.findByText('elsewhere')).toBeInTheDocument()
  })

  it('edits: loads the record with the document masked and the address in place, then puts to the uuid path', async () => {
    const put = vi.spyOn(api, 'put').mockResolvedValue({ data: {} } as never)
    renderPage(<CustomerFormPage />, '/customers/c1/edit', '/customers/:uuid/edit')
    expect(await screen.findByDisplayValue('Acme')).toBeInTheDocument()
    expect(taxId()).toHaveValue('11.222.333/0001-81')
    expect(screen.getByDisplayValue('Rua A, 10')).toBeInTheDocument()
    expect(screen.getByLabelText(/^status/i)).toHaveValue('Inactive')
    fireEvent.change(tradeName(), { target: { value: 'Acme Bus' } })
    fireEvent.click(screen.getByRole('button', { name: save }))
    await waitFor(() => expect(put).toHaveBeenCalledWith('/customer/uuid/c1', expect.objectContaining({ name: 'Acme Bus', legalName: 'Acme Ltda', taxId: '11222333000181', status: 'Inactive', administrativeArea: 'SP', locality: 'Campinas', postalCode: '13000000', countryCode: 'BR' })))
  })

  it('points a duplicate or invalid document the server refused at the field, not at the whole form', async () => {
    vi.spyOn(api, 'post').mockRejectedValueOnce(failure(409, 'DuplicateCustomerTaxId', 'Já existe um cliente com este documento.'))
    renderPage(<CustomerFormPage />, '/customers/new', '/customers/new')
    fireEvent.change(tradeName(), { target: { value: 'Acme' } })
    fireEvent.change(taxId(), { target: { value: '52998224725' } })
    fireEvent.click(screen.getByRole('button', { name: save }))
    const alert = await screen.findByRole('alert')
    expect(alert).toHaveTextContent('Já existe um cliente com este documento.')
    expect(alert.closest('label')).toContainElement(taxId())
    expect(screen.queryByText('elsewhere')).toBeNull()
  })

  it('shows any other server failure as the form error', async () => {
    vi.spyOn(api, 'post').mockRejectedValueOnce(failure(403, 'CustomerForbidden', 'Sem permissão.'))
    renderPage(<CustomerFormPage />, '/customers/new', '/customers/new')
    fireEvent.change(tradeName(), { target: { value: 'Acme' } })
    fireEvent.click(screen.getByRole('button', { name: save }))
    expect(await screen.findByRole('alert')).toHaveTextContent('Sem permissão.')
  })

  it('starts a new customer in the tenant country when the tenant is known', async () => {
    store.dispatch({ type: 'data/currentTenant/fulfilled', payload: { businessName: 'T', taxId: '1', countryCode: 'US' } })
    renderPage(<CustomerFormPage />, '/customers/new', '/customers/new')
    await waitFor(() => expect((screen.getByRole('radio', { name: /united states|estados unidos/i }) as HTMLInputElement).checked).toBe(true))
    expect(screen.getByLabelText(/EIN/)).toBeInTheDocument()
  })
})

describe('customer form mapping', () => {
  it('reads a record into the form with the document masked, and writes it back digits-only without blanks', () => {
    const values = fromCustomer({ name: 'Acme', status: 'Active', taxId: '52998224725', legalName: null, notes: null }, 'BR')
    expect(values).toMatchObject({ name: 'Acme', status: 'Active', taxId: '529.982.247-25', legalName: '', notes: '', countryCode: 'BR' })
    expect(toCustomerPayload({ ...values, taxId: '11.222.333/0001-81', notes: '  ' })).toEqual({ name: 'Acme', status: 'Active', taxId: '11222333000181', countryCode: 'BR' })
  })
})
