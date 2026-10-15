import { createAsyncThunk, createSlice, isAnyOf } from '@reduxjs/toolkit'
import { apiMessage } from '../api'
import * as referenceDataService from '../services/referenceDataService'
import type { ReferenceRow } from '../services/referenceDataService'
import type { Params } from '../services/resourceService'
import type { Page } from '../types'

const failure = (cause: unknown) => apiMessage(cause, '')

export const fetchReferenceRows = createAsyncThunk<Page<ReferenceRow>, { path: string; params: Params }, { rejectValue: string }>('referenceData/list', async (request, { rejectWithValue }) => {
  try { return await referenceDataService.listReferenceRows(request) } catch (cause) { return rejectWithValue(failure(cause)) }
})
export const saveReferenceRow = createAsyncThunk<void, { path: string; row: ReferenceRow }, { rejectValue: string }>('referenceData/save', async (request, { rejectWithValue }) => {
  try { await referenceDataService.saveReferenceRow(request) } catch (cause) { return rejectWithValue(failure(cause)) }
})
export const importReferenceRows = createAsyncThunk<{ created: number; updated: number }, { path: string; rows: ReferenceRow[] }, { rejectValue: string }>('referenceData/import', async (request, { rejectWithValue }) => {
  try { return await referenceDataService.importReferenceRows(request) } catch (cause) { return rejectWithValue(failure(cause)) }
})

const referenceDataSlice = createSlice({
  name: 'referenceData', initialState: { loading: false, saving: false, error: '' }, reducers: {},
  extraReducers: (builder) => builder
    .addMatcher(isAnyOf(fetchReferenceRows.pending), (state) => { state.loading = true; state.error = '' })
    .addMatcher(isAnyOf(fetchReferenceRows.fulfilled), (state) => { state.loading = false })
    .addMatcher(isAnyOf(fetchReferenceRows.rejected), (state, action) => { state.loading = false; state.error = action.payload || 'loadError' })
    .addMatcher(isAnyOf(saveReferenceRow.pending, importReferenceRows.pending), (state) => { state.saving = true; state.error = '' })
    .addMatcher(isAnyOf(saveReferenceRow.fulfilled, importReferenceRows.fulfilled), (state) => { state.saving = false })
    .addMatcher(isAnyOf(saveReferenceRow.rejected, importReferenceRows.rejected), (state, action) => { state.saving = false; state.error = action.payload || 'genericError' }),
})

export const referenceDataReducer = referenceDataSlice.reducer