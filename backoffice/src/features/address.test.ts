import { describe, expect, it } from 'vitest'
import { i18n } from '../i18n'
import { addressLines } from './address'

describe('address lines', () => {
  it('prints street, complement, "city · state" and "postal code · country", skipping blanks', () => {
    const t = i18n.getFixedT('en')
    expect(addressLines({ addressLine1: 'Rua A, 10', addressLine2: 'Sala 2', locality: 'Campinas', administrativeArea: 'SP', postalCode: '13000-000', countryCode: 'BR' }, t))
      .toEqual(['Rua A, 10', 'Sala 2', 'Campinas · SP', '13000-000 · Brazil'])
    expect(addressLines({ locality: 'Austin', countryCode: 'US' }, t)).toEqual(['Austin', 'United States'])
    expect(addressLines({}, t)).toEqual([])
  })
})
