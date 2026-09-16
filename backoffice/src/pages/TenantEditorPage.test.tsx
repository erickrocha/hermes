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

  it('renders BR and US radio buttons and switches countries', async () => {
    const apiGetSpy = vi.spyOn(api, 'get').mockImplementation(async (url: string) => {
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

    // Verify radio buttons
    const brRadio = screen.getByRole('radio', { name: /BR/i })
    const usRadio = screen.getByRole('radio', { name: /US/i })
    expect(brRadio).toBeInTheDocument()
    expect(usRadio).toBeInTheDocument()

    // Initially loads provinces for the default country (BR or US)
    await waitFor(() => {
      expect(apiGetSpy).toHaveBeenCalledWith('/province', expect.anything())
    })

    // Click US radio
    fireEvent.click(usRadio)
    expect(usRadio).toBeChecked()
    expect(brRadio).not.toBeChecked()

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
