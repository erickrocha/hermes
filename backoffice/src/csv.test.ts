import { describe, expect, it } from 'vitest'
import { matchColumns, parseCsv } from './csv'

// PD-027: o arquivo vem da planilha de alguém. Cada caso aqui é uma forma real
// de o Excel exportar algo que um parser ingênuo lê errado em silêncio.
describe('parseCsv', () => {
  it('reads a plain comma file', () => {
    const { headers, rows } = parseCsv('acronym,name,countryCode\nSP,São Paulo,BR')
    expect(headers).toEqual(['acronym', 'name', 'countryCode'])
    expect(rows).toEqual([['SP', 'São Paulo', 'BR']])
  })

  it('detects the semicolon that Brazilian and European Excel exports use', () => {
    const { headers, rows } = parseCsv('acronym;name;countryCode\nSP;São Paulo;BR')
    expect(headers).toEqual(['acronym', 'name', 'countryCode'])
    expect(rows).toEqual([['SP', 'São Paulo', 'BR']])
  })

  it('keeps a quoted delimiter inside its field instead of splitting on it', () => {
    const { rows } = parseCsv('name,note\n"São Paulo, SP",capital')
    expect(rows).toEqual([['São Paulo, SP', 'capital']])
  })

  it('reads a doubled quote as one literal quote', () => {
    const { rows } = parseCsv('name\n"He said ""hi"""')
    expect(rows).toEqual([['He said "hi"']])
  })

  it('strips the byte order mark Excel writes, so the first header still matches', () => {
    const { headers } = parseCsv('﻿acronym,name\nSP,São Paulo')
    expect(headers[0]).toBe('acronym')
  })

  it('pads a short row instead of dropping it, so the operator can fix it in the grid', () => {
    const { rows } = parseCsv('acronym,name,countryCode\nSP,São Paulo')
    expect(rows).toEqual([['SP', 'São Paulo', '']])
  })

  it('ignores blank lines and trailing newlines', () => {
    const { rows } = parseCsv('a,b\n1,2\n\n3,4\n')
    expect(rows).toEqual([['1', '2'], ['3', '4']])
  })

  it('handles CRLF line endings', () => {
    const { rows } = parseCsv('a,b\r\n1,2\r\n')
    expect(rows).toEqual([['1', '2']])
  })

  it('returns nothing for an empty file rather than throwing', () => {
    expect(parseCsv('')).toEqual({ headers: [], rows: [] })
  })
})

describe('matchColumns', () => {
  it('matches regardless of case, spacing, punctuation and accents', () => {
    const headers = ['Acronym', 'Full Name', 'country_code']
    expect(matchColumns(headers, ['acronym', 'name', 'countryCode'])).toEqual({
      acronym: 0,
      countryCode: 2,
    })
  })

  it('omits fields the file does not carry, rather than guessing a column', () => {
    expect(matchColumns(['name'], ['name', 'countryCode'])).toEqual({ name: 0 })
  })
})
