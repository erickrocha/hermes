import { describe, expect, it } from 'vitest'
import { buildSchema, defaultsFor, fromServer, toPayload, type FieldDef } from './fields'

const fields: FieldDef[] = [
  { name: 'name', labelKey: 'name', type: 'text', required: true },
  { name: 'qty', labelKey: 'quantity', type: 'number', min: 1 },
  { name: 'value', labelKey: 'amount', type: 'money' },
  { name: 'at', labelKey: 'date', type: 'datetime' },
  { name: 'start', labelKey: 'startTime', type: 'time' },
  { name: 'on', labelKey: 'activeFlag', type: 'checkbox' },
  { name: 'plate', labelKey: 'plate', type: 'text', createOnly: true },
]

describe('form fields', () => {
  it('turns the form into the request body: blanks vanish, numbers and money convert, times gain seconds', () => {
    const body = toPayload(fields, { name: ' Bus ', qty: '3', value: '12.34', at: '2026-10-01T08:30', start: '07:15', on: true, plate: 'ABC1D23' })
    expect(body).toEqual({ name: 'Bus', qty: 3, value: 1234, at: '2026-10-01T08:30:00', start: '07:15:00', on: true, plate: 'ABC1D23' })
    expect(toPayload(fields, { name: 'x', qty: '', value: '', at: '', start: '', on: false, plate: '' })).toEqual({ name: 'x', on: false })
  })

  it('leaves create-only fields out of an edit', () => {
    expect(toPayload(fields, { name: 'x', qty: '', value: '', at: '', start: '', on: false, plate: 'ABC1D23' }, true)).not.toHaveProperty('plate')
  })

  it('reads the record back into the form: cents become units and date-times fit the input', () => {
    const values = fromServer(fields, { name: 'Bus', qty: 3, value: 1234, at: '2026-10-01T08:30:15', on: true })
    expect(values).toMatchObject({ name: 'Bus', qty: '3', value: '12.34', at: '2026-10-01T08:30', on: true, start: '' })
  })

  it('validates required, numeric and minimum', async () => {
    const schema = buildSchema(fields)
    await expect(schema.validate(defaultsFor(fields), { abortEarly: false })).rejects.toThrow()
    await expect(schema.isValid({ ...defaultsFor(fields), name: 'x', qty: 'abc' })).resolves.toBe(false)
    await expect(schema.isValid({ ...defaultsFor(fields), name: 'x', qty: '0' })).resolves.toBe(false)
    await expect(schema.isValid({ ...defaultsFor(fields), name: 'x', qty: '2' })).resolves.toBe(true)
  })
})
