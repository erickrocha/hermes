import { createAsyncThunk, createSlice, isAnyOf } from '@reduxjs/toolkit'
import { apiMessage } from '../api'
import * as costService from '../services/costService'
import type { Params } from '../services/resourceService'
import type { SupplierClosing, VehicleCost, VehicleOption } from '../services/costService'

const failure = (cause: unknown) => apiMessage(cause, '')

export const fetchCostVehicles = createAsyncThunk<VehicleOption[], void, { rejectValue: string }>('cost/vehicles', async (_, { rejectWithValue }) => {
  try { return await costService.listCostVehicles() } catch (cause) { return rejectWithValue(failure(cause)) }
})
export const fetchSuppliers = createAsyncThunk<string[], void, { rejectValue: string }>('cost/suppliers', async (_, { rejectWithValue }) => {
  try { return await costService.listSuppliers() } catch (cause) { return rejectWithValue(failure(cause)) }
})
export const fetchVehicleCost = createAsyncThunk<VehicleCost, { vehicle: string; params: Params }, { rejectValue: string }>('cost/vehicle', async (request, { rejectWithValue }) => {
  try { return await costService.loadVehicleCost(request) } catch (cause) { return rejectWithValue(failure(cause)) }
})
export const fetchSupplierClosing = createAsyncThunk<SupplierClosing, Params, { rejectValue: string }>('cost/supplierClosing', async (params, { rejectWithValue }) => {
  try { return await costService.loadSupplierClosing(params) } catch (cause) { return rejectWithValue(failure(cause)) }
})
export const downloadCostReport = createAsyncThunk<void, { path: string; params: Params }, { rejectValue: string }>('cost/download', async (request, { rejectWithValue }) => {
  try { await costService.downloadCostReport(request) } catch (cause) { return rejectWithValue(failure(cause)) }
})

const costSlice = createSlice({
  name: 'cost', initialState: { loading: false, saving: false, error: '' }, reducers: {},
  extraReducers: (builder) => builder
    .addMatcher(isAnyOf(fetchCostVehicles.pending, fetchSuppliers.pending, fetchVehicleCost.pending, fetchSupplierClosing.pending), (state) => { state.loading = true; state.error = '' })
    .addMatcher(isAnyOf(fetchCostVehicles.fulfilled, fetchSuppliers.fulfilled, fetchVehicleCost.fulfilled, fetchSupplierClosing.fulfilled), (state) => { state.loading = false })
    .addMatcher(isAnyOf(fetchCostVehicles.rejected, fetchSuppliers.rejected, fetchVehicleCost.rejected, fetchSupplierClosing.rejected), (state, action) => { state.loading = false; state.error = action.payload || 'loadError' })
    .addMatcher(isAnyOf(downloadCostReport.pending), (state) => { state.saving = true; state.error = '' })
    .addMatcher(isAnyOf(downloadCostReport.fulfilled), (state) => { state.saving = false })
    .addMatcher(isAnyOf(downloadCostReport.rejected), (state, action) => { state.saving = false; state.error = action.payload || 'loadError' }),
})

export const costReducer = costSlice.reducer