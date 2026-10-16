import { createAsyncThunk, createSlice, isAnyOf } from '@reduxjs/toolkit'
import { apiMessage } from '../api'
import * as billingService from '../services/billingService'
import { emptyPage } from '../types'
import type { BillingCharge, BillingSettings, Page, PageRequest } from '../types'

const failure = (cause: unknown) => apiMessage(cause, '')

export const fetchBillingCharges = createAsyncThunk<Page<BillingCharge>, PageRequest, { rejectValue: string }>(
  'billing/charges', async (request, { rejectWithValue }) => {
    try { return await billingService.loadBillingCharges(request) } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)

export const fetchBillingSettings = createAsyncThunk<BillingSettings, string, { rejectValue: string }>(
  'billing/settings', async (tenantUuid, { rejectWithValue }) => {
    try { return await billingService.loadBillingSettings(tenantUuid) } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)

export const saveBillingSettings = createAsyncThunk<BillingSettings, { tenantUuid: string; graceDays: number }, { rejectValue: string }>(
  'billing/saveSettings', async (request, { rejectWithValue }) => {
    try { return await billingService.saveBillingSettings(request) } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)

export const setBillingSuspension = createAsyncThunk<BillingSettings, { tenantUuid: string; suspended: boolean }, { rejectValue: string }>(
  'billing/setSuspension', async (request, { rejectWithValue }) => {
    try { return await billingService.setBillingSuspension(request) } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)

export const confirmBillingCharge = createAsyncThunk<BillingCharge, string, { rejectValue: string }>(
  'billing/confirmCharge', async (uuid, { rejectWithValue }) => {
    try { return await billingService.confirmBillingCharge(uuid) } catch (cause) { return rejectWithValue(failure(cause)) }
  },
)

type BillingState = {
  charges: Page<BillingCharge>
  settings: BillingSettings | null
  settingsTenantUuid: string
  loadingCharges: boolean
  loadingSettings: boolean
  saving: boolean
  error: string
}

const initialState: BillingState = {
  charges: emptyPage<BillingCharge>(25),
  settings: null,
  settingsTenantUuid: '',
  loadingCharges: false,
  loadingSettings: false,
  saving: false,
  error: '',
}

const billingSlice = createSlice({
  name: 'billing', initialState, reducers: {},
  extraReducers: (builder) => builder
    .addCase(fetchBillingCharges.pending, (state) => { state.loadingCharges = true; state.error = '' })
    .addCase(fetchBillingCharges.fulfilled, (state, action) => { state.loadingCharges = false; state.charges = action.payload })
    .addCase(fetchBillingCharges.rejected, (state, action) => { state.loadingCharges = false; state.error = action.payload || 'loadError' })
    .addCase(fetchBillingSettings.pending, (state) => { state.loadingSettings = true; state.error = ''; state.settings = null })
    .addCase(fetchBillingSettings.fulfilled, (state, action) => { state.loadingSettings = false; state.settings = action.payload; state.settingsTenantUuid = action.payload.tenantUuid })
    .addCase(fetchBillingSettings.rejected, (state, action) => { state.loadingSettings = false; state.settingsTenantUuid = ''; state.error = action.payload || 'loadError' })
    .addCase(confirmBillingCharge.fulfilled, (state, action) => {
      state.saving = false
      state.charges.items = state.charges.items.map((charge) => charge.uuid === action.payload.uuid ? action.payload : charge)
    })
    .addMatcher(isAnyOf(saveBillingSettings.pending, setBillingSuspension.pending, confirmBillingCharge.pending), (state) => { state.saving = true; state.error = '' })
    .addMatcher(isAnyOf(saveBillingSettings.fulfilled, setBillingSuspension.fulfilled), (state, action) => { state.saving = false; state.settings = action.payload; state.settingsTenantUuid = action.payload.tenantUuid })
    .addMatcher(isAnyOf(saveBillingSettings.rejected, setBillingSuspension.rejected, confirmBillingCharge.rejected), (state, action) => { state.saving = false; state.error = action.payload || 'genericError' }),
})

export const billingReducer = billingSlice.reducer