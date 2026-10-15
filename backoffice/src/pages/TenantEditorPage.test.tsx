import { fireEvent, render, screen, waitFor } from '@testing-library/react'
import { Provider } from 'react-redux'
import { MemoryRouter, Route, Routes } from 'react-router-dom'
import { beforeEach, describe, expect, it, vi } from 'vitest'
import '../i18n'
import { api } from '../api'
import { TenantEditorPage } from './ManagementPages'
import { store } from '../store'

describe('TenantEditorPage Country, Province, and City selection', () => {
  beforeEach(() => {
    vi.restoreAllMocks()
  })

  // DEF-RD-08 (PD-027): the country list is whatever reference data has been
  // imported, so this asserts the form offers what `GET /country` returned --
  // a country onboarded by import must be selectable without a code change.
  it('offers the countries reference data actually has, and switches between them', async () => {
    const apiGetSpy = vi.spyOn(api, 'get').mockImplementation(async (url: string) => {
      if (url === '/country') return { data: ['BR', 'US', 'AR'] } as any
      if (url === '/province') {
        return {
          data: [
            { id: 1, acronym: 'SP', name: 'São Paulo', countryCode: 'BR' },
            { id: 2, acronym: 'RJ', name: 'Rio de Janeiro', countryCode: 'BR' },
          ],
        } as any
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

    const country = screen.getByLabelText(/country|país/i) as HTMLSelectElement
    // Argentina is in the list purely because its provinces were imported --
    // the old hardcoded BR/US pair could never have shown it.
    await waitFor(() => {
      expect(Array.from(country.options).map((option) => option.value)).toEqual(expect.arrayContaining(['BR', 'US', 'AR']))
    })

    await waitFor(() => {
      expect(apiGetSpy).toHaveBeenCalledWith('/province', expect.anything())
    })

    fireEvent.change(country, { target: { value: 'US' } })
    expect(country.value).toBe('US')

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
