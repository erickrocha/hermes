import { allResources } from './features/registry'
import { lookupReducer } from './features/lookupSlice'
import { panelReducer } from './features/panelSlice'
import { profileReducer } from './features/profileSlice'
import { managementReducer } from './features/managementSlice'
import { costReducer } from './features/costSlice'
import { referenceDataReducer } from './features/referenceDataSlice'
import { garageDeviceReducer } from './features/garageDeviceSlice'
import { maintenanceNotificationReducer } from './features/maintenanceNotificationSlice'
import { nonExecutionReducer } from './features/nonExecutionSlice'
import { billingReducer } from './features/billingSlice'
import { publicPlanReducer } from './features/publicPlanSlice'
import { configureStore, createAsyncThunk, createSlice } from '@reduxjs/toolkit'
import { TARGET_KEY, apiMessage, normalizeSession, readTarget } from './api'
import * as authService from './services/authService'
import * as dataService from './services/dataService'
import { emptyPage } from './types'
import { crossTenantActionReducer } from './features/crossTenantActionSlice'
import type { BusinessPlan, City, Page, Province, Session, TargetTenant, Tenant, User } from './types'

const loadStoredSession = (): Session | null => {
  const stored = localStorage.getItem('hermes.session')
  if (!stored) return null

  try {
    const session = normalizeSession(JSON.parse(stored) as Record<string, unknown>) as Session
    if (!session.accessToken) throw new Error('Stored session has no access token')
    localStorage.setItem('hermes.session', JSON.stringify(session))
    return session
  } catch {
    localStorage.removeItem('hermes.session')
    return null
  }
}

const initialSession = loadStoredSession()

export const login = createAsyncThunk('auth/login', async (credentials: { email: string; password: string }, { rejectWithValue }) => {
  try { return await authService.login(credentials) }
  catch (error) { return rejectWithValue(apiMessage(error, 'Invalid credentials')) }
})
export const loadPermissions = createAsyncThunk('auth/permissions', async () => authService.loadPermissions())
export const changePassword = createAsyncThunk('auth/password', async (payload: { currentPassword: string; newPassword: string }, { rejectWithValue }) => {
  try { await authService.changePassword(payload); return true } catch (error) { return rejectWithValue(apiMessage(error, 'Unable to change password')) }
})
export const acceptInvitation = createAsyncThunk('auth/acceptInvitation', async (payload: { token: string; newPassword: string }, { rejectWithValue }) => {
  try { await authService.acceptInvitation(payload) } catch (error) { return rejectWithValue(apiMessage(error, '')) }
})

/// C-036 (`DS-1`): act inside a descendant tenant (or, with `null`, back in the signed-in one). The target is stored
/// first, so the request that follows already carries it, and the effective permissions are read again -- inside a
/// descendant they are what the caller's `tenant.child.*` grants stand for there, and the menus follow them.
export const selectTargetTenant = createAsyncThunk('auth/target', async (target: TargetTenant | null, { dispatch }) => {
  if (target) localStorage.setItem(TARGET_KEY, JSON.stringify(target)); else localStorage.removeItem(TARGET_KEY)
  dispatch(authSlice.actions.setTarget(target))
  await dispatch(loadPermissions())
})

const authSlice = createSlice({
  name: 'auth', initialState: { session: initialSession, loading: false, error: '', target: initialSession ? readTarget() : null as TargetTenant | null },
  reducers: { setTarget: (state, action: { payload: TargetTenant | null }) => { state.target = action.payload }, logout: (state) => { state.session = null; state.target = null; localStorage.removeItem('hermes.session'); localStorage.removeItem(TARGET_KEY) }, clearAuthError: (state) => { state.error = '' } },
  extraReducers: (builder) => builder
    .addCase(login.pending, (state) => { state.loading = true; state.error = '' })
    .addCase(login.fulfilled, (state, action) => { state.loading = false; state.session = action.payload; state.target = null; localStorage.removeItem(TARGET_KEY); localStorage.setItem('hermes.session', JSON.stringify(action.payload)) })
    .addCase(loadPermissions.fulfilled, (state, action) => {
      // Only a well-formed answer replaces what the token brought.
      if (!state.session || !Array.isArray(action.payload?.permissions)) return
      const { permissions, scope, profileKey, profileName } = action.payload
      // Inside a descendant the answer is the delegated one; whether the account may act there is its own, set at sign-in
      // and worked out again only when no target is chosen.
      const canTarget = state.target ? state.session.canTarget : permissions.includes('tenant.child.data.read')
      state.session = { ...state.session, permissions, scope: scope === 'Platform' ? 'Platform' : 'Tenant', profileKey, profileName, canTarget }
      localStorage.setItem('hermes.session', JSON.stringify(state.session))
    })
    .addCase(login.rejected, (state, action) => { state.loading = false; state.error = String(action.payload ?? '') })
    .addCase(acceptInvitation.rejected, (state, action) => { state.error = String(action.payload ?? '') })
})

type Counts = { tenants: number; users: number; plans: number }
type DataState = { tenants: Page<Tenant>; plans: Page<BusinessPlan>; users: Page<User>; provinces: Province[]; cities: City[]; countries: string[]; counts: Counts; tenantDashboard: dataService.TenantDashboard; currentTenant: Tenant | null; loading: boolean; error: string }
const initialData: DataState = { tenants: emptyPage<Tenant>(), plans: emptyPage<BusinessPlan>(), users: emptyPage<User>(), provinces: [], cities: [], countries: [], counts: { tenants: 0, users: 0, plans: 0 }, tenantDashboard: dataService.emptyTenantDashboard(), currentTenant: null, loading: false, error: '' }
export const loadTenants = createAsyncThunk('data/tenants', dataService.fetchTenants)
export const loadPlans = createAsyncThunk('data/plans', dataService.fetchPlans)
export const loadUsers = createAsyncThunk('data/users', dataService.fetchUsers)
export const loadProvinces = createAsyncThunk('data/provinces', dataService.fetchProvinces)
export const loadCities = createAsyncThunk('data/cities', dataService.fetchCities)
// DEF-RD-08: the countries a tenant may be placed in come from the reference
// data that was actually imported, not from a hardcoded pair in the form.
export const loadCountries = createAsyncThunk('data/countries', dataService.fetchCountries)

// DEF-BO-07: the dashboard needs three totals, not three pages. It used to
// fetch `{page:0,pageSize:1}` into the very same `data.tenants|users|plans`
// slots the list screens read, so visiting the dashboard left the Tenants
// page showing "1-1 of 29 - Page 1 of 29". Counts now live in their own
// state, so the two screens cannot overwrite each other.
export const loadCounts = createAsyncThunk('data/counts', dataService.fetchCounts)
export const loadTenantDashboard = createAsyncThunk('data/tenantDashboard', dataService.fetchTenantDashboard)

// DEF-BO-02: the theme was derived from whichever tenant happened to be in
// the already-loaded tenant *list*, so a TenantOwner saw their identity only
// after some screen had listed tenants -- and never at all once the dashboard
// stopped loading that list. The session's own tenant is now fetched
// directly, which is the only thing the theme should ever depend on.
//
// HRMS-204/OBS-TP-05: addressed by the tenant's public uuid, which the login
// response now carries, so no console URL or API path uses the internal id.
export const loadCurrentTenant = createAsyncThunk('data/currentTenant', dataService.fetchCurrentTenant)

const dataSlice = createSlice({
  name: 'data', initialState: initialData, reducers: {
    clearDataError: (state) => { state.error = '' },
    clearCities: (state) => { state.cities = [] },
  },
  extraReducers: (builder) => {
    const listThunks = [loadTenants, loadPlans, loadUsers, loadProvinces, loadCities, loadCountries] as const
    listThunks.forEach((thunk) => {
      builder.addCase(thunk.pending, (state) => { state.loading = true; state.error = '' })
      builder.addCase(thunk.rejected, (state, action) => { state.loading = false; state.error = String(action.error.message ?? '') })
    })
    builder.addCase(loadTenants.fulfilled, (s, a) => { s.loading = false; s.tenants = a.payload })
    builder.addCase(loadPlans.fulfilled, (s, a) => { s.loading = false; s.plans = a.payload })
    builder.addCase(loadUsers.fulfilled, (s, a) => { s.loading = false; s.users = a.payload })
    builder.addCase(loadProvinces.fulfilled, (s, a) => { s.loading = false; s.provinces = a.payload; s.cities = [] })
    builder.addCase(loadCities.fulfilled, (s, a) => { s.loading = false; s.cities = a.payload })
    builder.addCase(loadCountries.fulfilled, (s, a) => { s.loading = false; s.countries = a.payload })
    builder.addCase(loadCounts.fulfilled, (s, a) => { s.counts = a.payload })
    builder.addCase(loadTenantDashboard.fulfilled, (s, a) => { s.tenantDashboard = a.payload })
    builder.addCase(loadCurrentTenant.fulfilled, (s, a) => { s.currentTenant = a.payload })
    // A tenant the operator just renamed should retheme the console without a
    // reload, and the counts card should not go stale after a create/delete.
    builder.addCase(loadCurrentTenant.rejected, (s) => { s.currentTenant = null })
  },
})

// Uma slice por feature (resourceSlice) mais os seletores (lookup): o registro único mantém o store e os testes de acordo.
const featureReducers = Object.fromEntries(allResources.map((resource) => [resource.name, resource.reducer]))
export const store = configureStore({ reducer: { auth: authSlice.reducer, data: dataSlice.reducer, lookup: lookupReducer, panel: panelReducer, profiles: profileReducer, management: managementReducer, billing: billingReducer, publicPlans: publicPlanReducer, cost: costReducer, referenceData: referenceDataReducer, garageDevices: garageDeviceReducer, maintenanceNotifications: maintenanceNotificationReducer, nonExecutions: nonExecutionReducer, crossTenantActions: crossTenantActionReducer, ...featureReducers } })
export const { logout, clearAuthError } = authSlice.actions
export const { clearDataError, clearCities } = dataSlice.actions
export type RootState = ReturnType<typeof store.getState>
export type AppDispatch = typeof store.dispatch
