import type { TFunction } from 'i18next'

/// The postal address every registry shares (tenant, customer): the fields `AddressFields` edits.
export interface AddressValues {
  addressLine1?: string | null
  addressLine2?: string | null
  locality?: string | null
  administrativeArea?: string | null
  postalCode?: string | null
  countryCode?: string | null
}

export const ADDRESS_FIELDS = ['addressLine1', 'addressLine2', 'locality', 'administrativeArea', 'postalCode', 'countryCode'] as const

export const countryLabelKey = (countryCode: string) => (countryCode === 'US' ? 'countryUnitedStates' : 'countryBrazil')

/// The address as the lines a card prints: street, complement, "city · state" (as the tenant card joins them), "postal code · country".
export const addressLines = (address: AddressValues, t: TFunction): string[] => {
  const place = [address.locality, address.administrativeArea].filter(Boolean).join(' · ')
  const region = [address.postalCode, address.countryCode ? t(countryLabelKey(address.countryCode)) : ''].filter(Boolean).join(' · ')
  return [address.addressLine1 ?? '', address.addressLine2 ?? '', place, region].map((line) => line.trim()).filter(Boolean)
}
