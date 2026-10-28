import * as yup from 'yup'
import type { LookupKind } from './lookupSlice'

export type FieldType = 'text' | 'number' | 'date' | 'datetime' | 'time' | 'textarea' | 'checkbox' | 'select' | 'lookup' | 'money' | 'list' | 'hidden' | 'password'
export interface FieldDef {
  name: string; labelKey: string; type: FieldType; required?: boolean; min?: number
  /** Texto: tamanho máximo, igual ao do servidor. */
  maxLength?: number
  options?: string[]; optionPrefix?: string; lookup?: LookupKind; defaultValue?: string | boolean
  /** Só no cadastro: o servidor não aceita mudar o campo depois. */
  createOnly?: boolean
  /** `list`: um grupo que se repete. Com `scalar`, cada linha é só um texto (o servidor espera uma lista de textos). */
  itemFields?: FieldDef[]; scalar?: boolean
  /** Aparece (desabilitado) mas não vai no corpo da requisição: serve de rótulo da linha. */
  transient?: boolean
  /** `list` com linhas fixas: sem adicionar nem remover. */
  fixedRows?: boolean
}
export type FormValue = string | boolean | Record<string, string | boolean>[]
export type FormValues = Record<string, FormValue>

const requiredText = 'required'

/// O esquema Yup nasce da própria lista de campos, então a tela e a validação não divergem.
export function buildSchema(fields: FieldDef[]) {
  const shape: Record<string, yup.AnySchema> = {}
  for (const field of fields) {
    if (field.type === 'checkbox') { shape[field.name] = yup.boolean(); continue }
    if (field.type === 'list') {
      let list = yup.array().of(buildSchema(field.itemFields ?? []) as never)
      if (field.required) list = list.min(1, requiredText)
      shape[field.name] = list; continue
    }
    let schema: yup.StringSchema = yup.string().trim()
    if (field.required) schema = schema.required(requiredText)
    if (field.maxLength !== undefined) schema = schema.max(field.maxLength, 'max')
    if (field.type === 'number' || field.type === 'money') {
      schema = schema.test('number', 'number', (value) => !value || !Number.isNaN(Number(value)))
      if (field.min !== undefined) { const min = field.min; schema = schema.test('min', 'min', (value) => !value || Number(value) >= min) }
    }
    shape[field.name] = schema
  }
  return yup.object(shape)
}

export const defaultsFor = (fields: FieldDef[]): FormValues =>
  Object.fromEntries(fields.map((field) => [field.name, field.defaultValue ?? (field.type === 'checkbox' ? false : field.type === 'list' ? [] : '')]))

/// Do que o servidor devolve para o que o formulário mostra (datas com hora cabem em `datetime-local`).
export function fromServer(fields: FieldDef[], data: Record<string, unknown>): FormValues {
  return Object.fromEntries(fields.map((field) => {
    const raw = data[field.name]
    if (raw === undefined || raw === null) return [field.name, field.type === 'checkbox' ? false : field.type === 'list' ? [] : '']
    if (field.type === 'list') return [field.name, (raw as unknown[]).map((entry) => fromServer(field.itemFields ?? [], field.scalar ? { value: entry } : (entry as Record<string, unknown>))) as Record<string, string | boolean>[]]
    if (field.type === 'checkbox') return [field.name, Boolean(raw)]
    if (field.type === 'datetime') return [field.name, String(raw).slice(0, 16)]
    if (field.type === 'money') return [field.name, String(Number(raw) / 100)]
    return [field.name, String(raw)]
  }))
}

/// Do formulário para o corpo da requisição: vazio some, número vira número, hora e data-hora ganham os segundos.
export function toPayload(fields: FieldDef[], values: FormValues, editing = false): Record<string, unknown> {
  const body: Record<string, unknown> = {}
  for (const field of fields) {
    if ((editing && field.createOnly) || field.transient) continue
    const value = values[field.name]
    if (field.type === 'list') {
      const rows = (value as Record<string, string | boolean>[] | undefined) ?? []
      body[field.name] = rows.map((row) => { const item = toPayload(field.itemFields ?? [], row); return field.scalar ? item.value : item })
      continue
    }
    if (field.type === 'checkbox') { body[field.name] = Boolean(value); continue }
    const text = String(value ?? '').trim()
    if (text === '') continue
    if (field.type === 'number') body[field.name] = Number(text)
    else if (field.type === 'money') body[field.name] = Math.round(Number(text) * 100)
    else if (field.type === 'datetime' || field.type === 'time') body[field.name] = text.length === (field.type === 'time' ? 5 : 16) ? `${text}:00` : text
    else body[field.name] = text
  }
  return body
}
