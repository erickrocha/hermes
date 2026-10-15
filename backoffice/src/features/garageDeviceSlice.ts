import { createAsyncThunk, createSlice, isAnyOf } from '@reduxjs/toolkit'
import { apiMessage } from '../api'
import * as garageDeviceService from '../services/garageDeviceService'
import type { GarageDevice, PairingCode } from '../services/garageDeviceService'

const failure = (cause: unknown) => apiMessage(cause, '')

export const fetchGarageDevices = createAsyncThunk<GarageDevice[], void, { rejectValue: string }>(
  'garageDevices/list', async (_, { rejectWithValue }) => {
    try { return await garageDeviceService.listGarageDevices() } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)
export const issueGarageDeviceCode = createAsyncThunk<PairingCode, string, { rejectValue: string }>(
  'garageDevices/issue', async (name, { rejectWithValue }) => {
    try { return await garageDeviceService.issueGarageDeviceCode(name) } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)
export const reissueGarageDeviceCode = createAsyncThunk<PairingCode, string, { rejectValue: string }>(
  'garageDevices/reissue', async (uuid, { rejectWithValue }) => {
    try { return await garageDeviceService.reissueGarageDeviceCode(uuid) } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)
export const revokeGarageDevice = createAsyncThunk<void, string, { rejectValue: string }>(
  'garageDevices/revoke', async (uuid, { rejectWithValue }) => {
    try { await garageDeviceService.revokeGarageDevice(uuid) } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)

const initialState = { items: [] as GarageDevice[], loading: false, saving: false, error: '' }
const garageDeviceSlice = createSlice({
  name: 'garageDevices', initialState, reducers: {},
  extraReducers: (builder) => builder
    .addCase(fetchGarageDevices.pending, (state) => { state.loading = true; state.error = '' })
    .addCase(fetchGarageDevices.fulfilled, (state, action) => { state.loading = false; state.items = action.payload })
    .addCase(fetchGarageDevices.rejected, (state, action) => { state.loading = false; state.error = action.payload || 'loadError' })
    .addMatcher(isAnyOf(issueGarageDeviceCode.pending, reissueGarageDeviceCode.pending, revokeGarageDevice.pending), (state) => { state.saving = true; state.error = '' })
    .addMatcher(isAnyOf(issueGarageDeviceCode.fulfilled, reissueGarageDeviceCode.fulfilled, revokeGarageDevice.fulfilled), (state) => { state.saving = false })
    .addMatcher(isAnyOf(issueGarageDeviceCode.rejected, reissueGarageDeviceCode.rejected, revokeGarageDevice.rejected), (state, action) => { state.saving = false; state.error = action.payload || 'genericError' }),
})

export const garageDeviceReducer = garageDeviceSlice.reducer