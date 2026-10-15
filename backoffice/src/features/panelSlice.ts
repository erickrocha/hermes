import { createAsyncThunk, createSlice } from '@reduxjs/toolkit'
import { apiMessage } from '../api'
import { read, type Params } from '../services/resourceService'

// Leituras avulsas de uma tela (histórico de quilometragem, relatório, medidor…): o slice guarda o resultado por chave.
export interface PanelState { data: unknown; loading: boolean; error: string }
export interface PanelRequest { key: string; path: string; params?: Params }

export const loadPanel = createAsyncThunk<{ key: string; data: unknown }, PanelRequest, { rejectValue: { key: string; message: string } }>(
  'panel/load', async ({ key, path, params }, { rejectWithValue }) => {
    try { return { key, data: await read<unknown>(path, params) } } catch (cause) { return rejectWithValue({ key, message: apiMessage(cause, '') || 'loadError' }) }
  })

const panelSlice = createSlice({
  name: 'panel', initialState: {} as Record<string, PanelState>,
  reducers: { clearPanel: (state, action: { payload: string }) => { delete state[action.payload] } },
  extraReducers: (builder) => builder
    .addCase(loadPanel.pending, (state, action) => { state[action.meta.arg.key] = { data: state[action.meta.arg.key]?.data, loading: true, error: '' } })
    .addCase(loadPanel.fulfilled, (state, action) => { state[action.payload.key] = { data: action.payload.data, loading: false, error: '' } })
    .addCase(loadPanel.rejected, (state, action) => { const key = action.meta.arg.key; state[key] = { data: undefined, loading: false, error: action.payload?.message ?? 'loadError' } }),
})
export const panelReducer = panelSlice.reducer
export const { clearPanel } = panelSlice.actions
