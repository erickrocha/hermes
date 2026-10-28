import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { FormProvider, useForm } from 'react-hook-form'
import { Provider } from 'react-redux'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import '../../i18n'
import { api } from '../../api'
import type { AddressValues } from '../../features/address'
import { store } from '../../store'
import { AddressFields } from './AddressFields'

const provinces = [{ id: 1, acronym: 'SP', name: 'São Paulo', countryCode: 'BR' }, { id: 2, acronym: 'RJ', name: 'Rio de Janeiro', countryCode: 'BR' }]
const cities = [{ id: 10, provinceId: 1, name: 'Campinas' }, { id: 11, provinceId: 1, name: 'Santos' }]

function Harness({ defaults, onValues }: { defaults: AddressValues; onValues?: (values: AddressValues) => void }) {
  const form = useForm<AddressValues>({ defaultValues: defaults })
  return <Provider store={store}><FormProvider {...form}><form onSubmit={form.handleSubmit((values) => onValues?.(values))}><AddressFields /><button type="submit">go</button></form></FormProvider></Provider>
}

describe('AddressFields', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
    store.dispatch({ type: 'data/clearCities' })
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => {
      if (url === '/province') return { data: provinces } as never
      if (url === '/cities/by-province/1') return { data: cities } as never
      return { data: [] } as never
    })
  })

  it('renders the address inputs and loads the provinces of the form country', async () => {
    const get = vi.mocked(api.get)
    render(<Harness defaults={{ countryCode: 'BR' }} />)
    expect(screen.getByLabelText(/^address$|^endereço$|^dirección$/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/address line 2|complemento|dirección línea 2/i)).toBeInTheDocument()
    expect(screen.getByLabelText(/postal code|^cep$|código postal/i)).toBeInTheDocument()
    expect((screen.getByRole('radio', { name: /brazil|brasil/i }) as HTMLInputElement).checked).toBe(true)
    await waitFor(() => expect(get).toHaveBeenCalledWith('/province', expect.objectContaining({ params: expect.objectContaining({ countryCode: 'BR' }) })))
  })

  it('cascades: picking a province loads its cities, and the city then becomes selectable', async () => {
    const get = vi.mocked(api.get)
    const onValues = vi.fn()
    render(<Harness defaults={{ countryCode: 'BR' }} onValues={onValues} />)
    const province = await screen.findByRole('combobox', { name: /province|estado|provincia/i })
    await waitFor(() => expect(province.querySelector('input')).not.toBeDisabled())
    fireEvent.focus(province.querySelector('input')!)
    fireEvent.click(await screen.findByRole('option', { name: /São Paulo \(SP\)/ }))
    await waitFor(() => expect(get).toHaveBeenCalledWith('/cities/by-province/1'))
    const city = screen.getByRole('combobox', { name: /city|cidade|ciudad/i })
    await waitFor(() => expect(city.querySelector('input')).not.toBeDisabled())
    fireEvent.focus(city.querySelector('input')!)
    fireEvent.click(await screen.findByRole('option', { name: 'Campinas' }))
    fireEvent.change(screen.getByLabelText(/postal code|^cep$|código postal/i), { target: { value: '13000000' } })
    fireEvent.click(screen.getByText('go'))
    await waitFor(() => expect(onValues).toHaveBeenCalledWith(expect.objectContaining({ administrativeArea: 'SP', locality: 'Campinas', postalCode: '13000-000', countryCode: 'BR' })))
  })

  it('switching the country clears the province and city and reloads the provinces', async () => {
    const get = vi.mocked(api.get)
    const onValues = vi.fn()
    render(<Harness defaults={{ countryCode: 'BR', administrativeArea: 'SP', locality: 'Campinas' }} onValues={onValues} />)
    await waitFor(() => expect(get).toHaveBeenCalledWith('/cities/by-province/1'))
    fireEvent.click(screen.getByRole('radio', { name: /united states|estados unidos/i }))
    await waitFor(() => expect(get).toHaveBeenCalledWith('/province', expect.objectContaining({ params: expect.objectContaining({ countryCode: 'US' }) })))
    fireEvent.click(screen.getByText('go'))
    await waitFor(() => expect(onValues).toHaveBeenCalledWith(expect.objectContaining({ administrativeArea: '', locality: '', countryCode: 'US' })))
  })
})
