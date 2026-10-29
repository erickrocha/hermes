import { createAsyncThunk, createSlice } from '@reduxjs/toolkit'
import { apiMessage } from '../api'
import * as crossTenantActionService from '../services/crossTenantActionService'
import { emptyPage } from '../types'
import type { CrossTenantAction, Page, PageRequest } from '../types'

export const loadCrossTenantActions = createAsyncThunk<Page<CrossTenantAction>, PageRequest, { rejectValue: string }>(
  'crossTenantActions/list', async (request, { rejectWithValue }) => {
    try { return await crossTenantActionService.fetchCrossTenantActions(request) } catch (cause) { return rejectWithValue(apiMessage(cause, '')) }
  },
)

const crossTenantActionSlice = createSlice({
  name: 'crossTenantActions', initialState: { page: emptyPage<CrossTenantAction>(), loading: false, error: '' }, reducers: {},
  extraReducers: (builder) => builder
    .addCase(loadCrossTenantActions.pending, (state) => { state.loading = true; state.error = '' })
    .addCase(loadCrossTenantActions.fulfilled, (state, action) => { state.loading = false; state.page = action.payload })
    .addCase(loadCrossTenantActions.rejected, (state, action) => { state.loading = false; state.error = action.payload || 'loadError' }),
})

export const crossTenantActionReducer = crossTenantActionSlice.reducer
