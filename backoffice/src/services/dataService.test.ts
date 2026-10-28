import { afterEach, describe, expect, it, vi } from 'vitest'
import { api } from '../api'
import { fetchCities, fetchCountries, fetchProvinces } from './dataService'

describe('reference lists', () => {
  afterEach(() => vi.restoreAllMocks())

  it('passes arrays through and reads any other body as an empty list', async () => {
    const provinces = [{ id: 1, acronym: 'SP', name: 'São Paulo', countryCode: 'BR' }]
    vi.spyOn(api, 'get').mockImplementation(async (url: string) => {
      if (url === '/province') return { data: provinces } as never
      if (url.startsWith('/cities')) return { data: { items: [], page: 0 } } as never
      return { data: null } as never
    })
    expect(await fetchProvinces('BR')).toEqual(provinces)
    expect(await fetchCities(1)).toEqual([])
    expect(await fetchCountries()).toEqual([])
  })
})
