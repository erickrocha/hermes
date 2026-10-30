import { createAsyncThunk, createSlice, isAnyOf } from '@reduxjs/toolkit'
import { apiMessage } from '../api'
import * as managementService from '../services/managementService'
import type { SubscriptionDetails } from '../services/managementService'
import type { BusinessPlan, Tenant, User } from '../types'

const failure = (cause: unknown) => apiMessage(cause, '')

export const fetchTenant = createAsyncThunk<Tenant, string, { rejectValue: string }>('management/tenant', async (uuid, { rejectWithValue }) => {
  try { return await managementService.loadTenant(uuid) } catch (cause) { return rejectWithValue(failure(cause)) }
})
export const saveTenant = createAsyncThunk<void, { uuid?: string; tenant: Tenant }, { rejectValue: string }>('management/saveTenant', async (request, { rejectWithValue }) => {
  try { await managementService.saveTenant(request) } catch (cause) { return rejectWithValue(failure(cause)) }
})
export const fetchSubscription = createAsyncThunk<SubscriptionDetails, string, { rejectValue: string }>('management/subscription', async (uuid, { rejectWithValue }) => {
  try { return await managementService.loadSubscription(uuid) } catch (cause) { return rejectWithValue(failure(cause)) }
})
export const saveSubscription = createAsyncThunk<void, { uuid: string; businessPlanId: number }, { rejectValue: string }>('management/saveSubscription', async (request, { rejectWithValue }) => {
  try { await managementService.saveSubscription(request) } catch (cause) { return rejectWithValue(failure(cause)) }
})
export const removePlan = createAsyncThunk<void, string, { rejectValue: string }>('management/removePlan', async (uuid, { rejectWithValue }) => {
  try { await managementService.deletePlan(uuid) } catch (cause) { return rejectWithValue(failure(cause)) }
})
export const fetchPlan = createAsyncThunk<BusinessPlan, string, { rejectValue: string }>('management/plan', async (uuid, { rejectWithValue }) => {
  try { return await managementService.loadPlan(uuid) } catch (cause) { return rejectWithValue(failure(cause)) }
})
export const saveBusinessPlan = createAsyncThunk<void, { uuid?: string; plan: BusinessPlan }, { rejectValue: string }>('management/savePlan', async (request, { rejectWithValue }) => {
  try { await managementService.savePlan(request) } catch (cause) { return rejectWithValue(failure(cause)) }
})
export const toggleUser = createAsyncThunk<void, { user: User; enabled: boolean }, { rejectValue: string }>('management/toggleUser', async (request, { rejectWithValue }) => {
  try { await managementService.setUserEnabled(request) } catch (cause) { return rejectWithValue(failure(cause)) }
})
export const fetchUser = createAsyncThunk<User, string, { rejectValue: string }>('management/user', async (uuid, { rejectWithValue }) => {
  try { return await managementService.loadUser(uuid) } catch (cause) { return rejectWithValue(failure(cause)) }
})
export const sendUserInvite = createAsyncThunk<void, string, { rejectValue: string }>('management/invite', async (uuid, { rejectWithValue }) => {
  try { await managementService.resendUserInvite(uuid) } catch (cause) { return rejectWithValue(failure(cause)) }
})
export const saveManagedUser = createAsyncThunk<void, { uuid?: string; user: User; tenantUuid?: string }, { rejectValue: string }>('management/saveUser', async (request, { rejectWithValue }) => {
  try { await managementService.saveUser(request) } catch (cause) { return rejectWithValue(failure(cause)) }
})

const managementSlice = createSlice({
  name: 'management', initialState: { loading: false, saving: false, error: '' }, reducers: {},
  extraReducers: (builder) => builder
    .addMatcher(isAnyOf(fetchTenant.pending, fetchSubscription.pending, fetchPlan.pending, fetchUser.pending), (state) => { state.loading = true; state.error = '' })
    .addMatcher(isAnyOf(fetchTenant.fulfilled, fetchSubscription.fulfilled, fetchPlan.fulfilled, fetchUser.fulfilled), (state) => { state.loading = false })
    .addMatcher(isAnyOf(fetchTenant.rejected, fetchSubscription.rejected, fetchPlan.rejected, fetchUser.rejected), (state, action) => { state.loading = false; state.error = action.payload || 'loadError' })
    .addMatcher(isAnyOf(saveTenant.pending, saveSubscription.pending, removePlan.pending, saveBusinessPlan.pending, toggleUser.pending, sendUserInvite.pending, saveManagedUser.pending), (state) => { state.saving = true; state.error = '' })
    .addMatcher(isAnyOf(saveTenant.fulfilled, saveSubscription.fulfilled, removePlan.fulfilled, saveBusinessPlan.fulfilled, toggleUser.fulfilled, sendUserInvite.fulfilled, saveManagedUser.fulfilled), (state) => { state.saving = false })
    .addMatcher(isAnyOf(saveTenant.rejected, saveSubscription.rejected, removePlan.rejected, saveBusinessPlan.rejected, toggleUser.rejected, sendUserInvite.rejected, saveManagedUser.rejected), (state, action) => { state.saving = false; state.error = action.payload || 'genericError' }),
})

export const managementReducer = managementSlice.reducer