import { describe, expect, it } from 'vitest'
import { isValidCpf, isValidCpfOrCnpj, isValidDocument, maskCpf, maskCpfOrCnpj, maskDocument, maskTaxId } from './masks'

describe('CPF check digits', () => {
  it('accepts a CPF whose two check digits match', () => {
    expect(isValidCpf('529.982.247-25')).toBe(true)
    expect(isValidCpf('52998224725')).toBe(true)
  })
  it('rejects a wrong check digit, a wrong length, and repeated digits', () => {
    expect(isValidCpf('529.982.247-24')).toBe(false)
    expect(isValidCpf('5299822472')).toBe(false)
    expect(isValidCpf('111.111.111-11')).toBe(false)
    expect(isValidCpf('')).toBe(false)
  })
  it('tells a CPF from a CNPJ by length', () => {
    expect(isValidCpfOrCnpj('529.982.247-25')).toBe(true)
    expect(isValidCpfOrCnpj('11.222.333/0001-81')).toBe(true)
    expect(isValidCpfOrCnpj('11.222.333/0001-82')).toBe(false)
    expect(isValidCpfOrCnpj('1234')).toBe(false)
  })
})

describe('document masks', () => {
  it('masks a CPF progressively and switches to the CNPJ mask past eleven digits', () => {
    expect(maskCpf('529')).toBe('529')
    expect(maskCpf('5299822')).toBe('529.982.2')
    expect(maskCpf('52998224725')).toBe('529.982.247-25')
    expect(maskCpfOrCnpj('52998224725')).toBe('529.982.247-25')
    expect(maskCpfOrCnpj('529982247251')).toBe('52.998.224/7251')
    expect(maskCpfOrCnpj('11222333000181')).toBe('11.222.333/0001-81')
    expect(maskCpfOrCnpj('112223330001819999')).toBe('11.222.333/0001-81')
  })
  it('masks by country and kind: a company field never reads as a CPF, the US uses the EIN mask', () => {
    expect(maskDocument('52998224725', 'BR')).toBe('529.982.247-25')
    expect(maskDocument('52998224725', 'BR', 'company')).toBe('52.998.224/725')
    expect(maskDocument('123456789', 'US')).toBe('12-3456789')
    expect(maskTaxId('11222333000181', 'BR')).toBe('11.222.333/0001-81')
    expect(maskTaxId('123456789', 'US')).toBe('12-3456789')
  })
  it('validates by country: CPF or CNPJ in Brazil, nine digits in the US', () => {
    expect(isValidDocument('529.982.247-25', 'BR')).toBe(true)
    expect(isValidDocument('529.982.247-25', 'BR', 'company')).toBe(false)
    expect(isValidDocument('12-3456789', 'US')).toBe(true)
    expect(isValidDocument('12-345678', 'US')).toBe(false)
  })
})
