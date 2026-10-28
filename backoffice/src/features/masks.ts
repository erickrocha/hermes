// Input masks and check-digit rules for documents, phones and postal codes, shared by every
// form that collects an identity or an address (tenant, customer).

export const digitsOnly = (value: string) => value.replace(/\D/g, '')

export const maskCnpj = (value: string) => {
  const digits = digitsOnly(value).slice(0, 14)
  return digits
    .replace(/^(\d{2})(\d)/, '$1.$2')
    .replace(/^(\d{2})\.(\d{3})(\d)/, '$1.$2.$3')
    .replace(/\.(\d{3})(\d)/, '.$1/$2')
    .replace(/(\d{4})(\d)/, '$1-$2')
}

export const maskCpf = (value: string) => {
  const digits = digitsOnly(value).slice(0, 11)
  return digits
    .replace(/^(\d{3})(\d)/, '$1.$2')
    .replace(/^(\d{3})\.(\d{3})(\d)/, '$1.$2.$3')
    .replace(/\.(\d{3})(\d)/, '.$1-$2')
}

/// A Brazilian document typed freely: up to 11 digits reads as a CPF, past that as a CNPJ.
export const maskCpfOrCnpj = (value: string) => {
  const digits = digitsOnly(value).slice(0, 14)
  return digits.length <= 11 ? maskCpf(digits) : maskCnpj(digits)
}

const maskEin = (value: string) => digitsOnly(value).slice(0, 9).replace(/^(\d{2})(\d)/, '$1-$2')

const checkDigit = (base: string, weights: number[]) => {
  const sum = [...base].reduce((total, digit, index) => total + Number(digit) * weights[index], 0)
  const remainder = sum % 11
  return remainder < 2 ? 0 : 11 - remainder
}

export const isValidCnpj = (value: string) => {
  const digits = digitsOnly(value)
  if (digits.length !== 14 || /^([0-9])\1+$/.test(digits)) return false
  const first = checkDigit(digits.slice(0, 12), [5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2])
  const second = checkDigit(digits.slice(0, 12) + first, [6, 5, 4, 3, 2, 9, 8, 7, 6, 5, 4, 3, 2])
  return digits.endsWith(`${first}${second}`)
}

export const isValidCpf = (value: string) => {
  const digits = digitsOnly(value)
  if (digits.length !== 11 || /^([0-9])\1+$/.test(digits)) return false
  const first = checkDigit(digits.slice(0, 9), [10, 9, 8, 7, 6, 5, 4, 3, 2])
  const second = checkDigit(digits.slice(0, 9) + first, [11, 10, 9, 8, 7, 6, 5, 4, 3, 2])
  return digits.endsWith(`${first}${second}`)
}

export const isValidCpfOrCnpj = (value: string) => {
  const digits = digitsOnly(value)
  return digits.length === 11 ? isValidCpf(digits) : isValidCnpj(digits)
}

/// Which documents a field accepts: a company's registration only, or a person's as well.
export type TaxIdKind = 'company' | 'any'

/// The tenant's document: CNPJ in Brazil, EIN in the United States.
export const maskTaxId = (value: string, countryCode: string) => (countryCode === 'BR' ? maskCnpj(value) : maskEin(value))

/// A document by country and kind -- the one display and input share.
export const maskDocument = (value: string, countryCode: string, kind: TaxIdKind = 'any') => {
  if (countryCode !== 'BR') return maskEin(value)
  return kind === 'company' ? maskCnpj(value) : maskCpfOrCnpj(value)
}

export const isValidDocument = (value: string, countryCode: string, kind: TaxIdKind = 'any') => {
  if (countryCode !== 'BR') return digitsOnly(value).length === 9
  return kind === 'company' ? isValidCnpj(value) : isValidCpfOrCnpj(value)
}

export const maskPhone = (value: string, countryCode: string) => {
  let digits = digitsOnly(value)
  if (countryCode === 'US') {
    if (digits.length > 10 && digits.startsWith('1')) digits = digits.slice(1)
    digits = digits.slice(0, 10)
    if (digits.length <= 3) return digits ? `(${digits}` : ''
    if (digits.length <= 6) return `(${digits.slice(0, 3)}) ${digits.slice(3)}`
    return `(${digits.slice(0, 3)}) ${digits.slice(3, 6)}-${digits.slice(6)}`
  }
  digits = digits.slice(0, 11)
  if (digits.length <= 2) return digits ? `(${digits}` : ''
  const subscriber = digits.slice(2)
  if (subscriber.length <= 4) return `(${digits.slice(0, 2)}) ${subscriber}`
  const split = subscriber.length > 8 ? 5 : 4
  return `(${digits.slice(0, 2)}) ${subscriber.slice(0, split)}-${subscriber.slice(split)}`
}

export const maskPostalCode = (value: string, countryCode: string) => {
  const digits = digitsOnly(value).slice(0, countryCode === 'BR' ? 8 : 9)
  return digits.length > 5 ? `${digits.slice(0, 5)}-${digits.slice(5)}` : digits
}
