import { createAsyncThunk, createSlice } from '@reduxjs/toolkit'
import { listPage } from '../services/resourceService'

// Listas curtas que alimentam os seletores dos formulários (veículo, cliente, peça, motorista, modelo).
export type LookupKind = 'vehicle' | 'customer' | 'part' | 'driver' | 'checklistTemplate' | 'garageService' | 'fuelEntry' | 'workOrder' | 'inspectionModelName'
export interface LookupOption { value: string; label: string }

interface Row { number?: string; description?: string; recordedAt?: string; station?: string; volumeLiters?: number; uuid?: string; plate?: string; model?: string; name?: string; code?: string; role?: string }
const sources: Record<LookupKind, { path: string; label: (row: Row) => string; keep?: (row: Row) => boolean; value?: (row: Row) => string }> = {
  vehicle: { path: '/vehicle', label: (r) => (r.model ? `${r.plate} · ${r.model}` : String(r.plate)) },
  customer: { path: '/customer', label: (r) => String(r.name) },
  part: { path: '/part', label: (r) => `${r.code ?? ''} ${r.name}`.trim() },
  driver: { path: '/user', label: (r) => String(r.name), keep: (r) => r.role === 'Driver' },
  checklistTemplate: { path: '/checklist-template', label: (r) => String(r.name) },
  garageService: { path: '/garage-service', label: (r) => String(r.name) },
  inspectionModelName: { path: '/inspection-model', label: (r) => String(r.name), value: (r) => String(r.name) },
  workOrder: { path: '/work-order', label: (r) => `${r.number ?? ''} — ${r.description ?? ''}` },
  fuelEntry: { path: '/fuel-entry', label: (r) => `${String(r.recordedAt ?? '').slice(0, 16).replace('T', ' ')} · ${r.volumeLiters ?? ''} l${r.station ? ` · ${r.station}` : ''}` },
}

export const loadLookup = createAsyncThunk('lookup/load', async (kind: LookupKind): Promise<{ kind: LookupKind; options: LookupOption[] }> => {
  const { path, label, keep, value } = sources[kind]
  const page = await listPage<Row>(path, { page: 0, pageSize: 200 })
  return { kind, options: page.items.filter((row) => row.uuid && (!keep || keep(row))).map((row) => ({ value: value ? value(row) : String(row.uuid), label: label(row) })) }
})

const lookupSlice = createSlice({
  name: 'lookup', initialState: {} as Partial<Record<LookupKind, LookupOption[]>>, reducers: {},
  extraReducers: (builder) => { builder.addCase(loadLookup.fulfilled, (state, action) => { state[action.payload.kind] = action.payload.options }) },
})
export const lookupReducer = lookupSlice.reducer
