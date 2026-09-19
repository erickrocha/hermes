import { configureStore, createAsyncThunk, createSlice } from '@reduxjs/toolkit'
import { api, apiMessage, normalizeSession } from './api'
import { emptyPage } from './types'
import type { BusinessPlan, City, Page, PageRequest, Province, Session, Tenant, User } from './types'

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
  try { const body = new URLSearchParams(credentials); const { data } = await api.post('/login', body, { headers: { 'Content-Type': 'application/x-www-form-urlencoded' } }); return normalizeSession(data) as Session }
  catch (error) { return rejectWithValue(apiMessage(error, 'Invalid credentials')) }
})
export const changePassword = createAsyncThunk('auth/password', async (payload: { currentPassword: string; newPassword: string }, { rejectWithValue }) => {
  try { await api.put('/user/change-password', payload); return true } catch (error) { return rejectWithValue(apiMessage(error, 'Unable to change password')) }
})

const authSlice = createSlice({
  name: 'auth', initialState: { session: initialSession, loading: false, error: '' },
  reducers: { logout: (state) => { state.session = null; localStorage.removeItem('hermes.session') }, clearAuthError: (state) => { state.error = '' } },
  extraReducers: (builder) => builder
    .addCase(login.pending, (state) => { state.loading = true; state.error = '' })
    .addCase(login.fulfilled, (state, action) => { state.loading = false; state.session = action.payload; localStorage.setItem('hermes.session', JSON.stringify(action.payload)) })
    .addCase(login.rejected, (state, action) => { state.loading = false; state.error = String(action.payload ?? '') })
})

type Counts = { tenants: number; users: number; plans: number }
type DataState = { tenants: Page<Tenant>; plans: Page<BusinessPlan>; users: Page<User>; provinces: Province[]; cities: City[]; countries: string[]; counts: Counts; currentTenant: Tenant | null; loading: boolean; error: string }
const initialData: DataState = { tenants: emptyPage<Tenant>(), plans: emptyPage<BusinessPlan>(), users: emptyPage<User>(), provinces: [], cities: [], countries: [], counts: { tenants: 0, users: 0, plans: 0 }, currentTenant: null, loading: false, error: '' }
const pageParams = ({ page, pageSize, search }: PageRequest) => ({ params: { page, pageSize, search: search || undefined } })

export const loadTenants = createAsyncThunk('data/tenants', async (request: PageRequest) => (await api.get<Page<Tenant>>('/tenant', pageParams(request))).data)
export const loadPlans = createAsyncThunk('data/plans', async (request: PageRequest) => (await api.get<Page<BusinessPlan>>('/business-plan', pageParams(request))).data)
export const loadUsers = createAsyncThunk('data/users', async (request: PageRequest) => (await api.get<Page<User>>('/user', pageParams(request))).data)
export const loadProvinces = createAsyncThunk('data/provinces', async (countryCode: string) => (await api.get<Province[]>('/province', { params: { countryCode, country_code: countryCode } })).data)
export const loadCities = createAsyncThunk('data/cities', async (provinceId: number) => (await api.get<City[]>(`/cities/by-province/${provinceId}`)).data)
// DEF-RD-08: the countries a tenant may be placed in come from the reference
// data that was actually imported, not from a hardcoded pair in the form.
export const loadCountries = createAsyncThunk('data/countries', async () => (await api.get<string[]>('/country')).data)

// DEF-BO-07: the dashboard needs three totals, not three pages. It used to
// fetch `{page:0,pageSize:1}` into the very same `data.tenants|users|plans`
// slots the list screens read, so visiting the dashboard left the Tenants
// page showing "1-1 of 29 - Page 1 of 29". Counts now live in their own
// state, so the two screens cannot overwrite each other.
export const loadCounts = createAsyncThunk('data/counts', async (includePlans: boolean) => {
  const countRequest = { params: { page: 0, pageSize: 1 } }
  const [tenants, users, plans] = await Promise.all([
    api.get<Page<Tenant>>('/tenant', countRequest),
    api.get<Page<User>>('/user', countRequest),
    includePlans ? api.get<Page<BusinessPlan>>('/business-plan', countRequest) : Promise.resolve(null),
  ])
  return { tenants: tenants.data.totalItems, users: users.data.totalItems, plans: plans ? plans.data.totalItems : 0 }
})

// DEF-BO-02: the theme was derived from whichever tenant happened to be in
// the already-loaded tenant *list*, so a TenantOwner saw their identity only
// after some screen had listed tenants -- and never at all once the dashboard
// stopped loading that list. The session's own tenant is now fetched
// directly, which is the only thing the theme should ever depend on.
export const loadCurrentTenant = createAsyncThunk('data/currentTenant', async (tenantId: number) => (await api.get<Tenant>(`/tenant/${tenantId}`)).data)

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
    builder.addCase(loadCurrentTenant.fulfilled, (s, a) => { s.currentTenant = a.payload })
    // A tenant the operator just renamed should retheme the console without a
    // reload, and the counts card should not go stale after a create/delete.
    builder.addCase(loadCurrentTenant.rejected, (s) => { s.currentTenant = null })
  },
})

export const store = configureStore({ reducer: { auth: authSlice.reducer, data: dataSlice.reducer } })
export const { logout, clearAuthError } = authSlice.actions
export const { clearDataError, clearCities } = dataSlice.actions
export type RootState = ReturnType<typeof store.getState>
export type AppDispatch = typeof store.dispatch
