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

type DataState = { tenants: Page<Tenant>; plans: Page<BusinessPlan>; users: Page<User>; provinces: Province[]; cities: City[]; loading: boolean; error: string }
const initialData: DataState = { tenants: emptyPage<Tenant>(), plans: emptyPage<BusinessPlan>(), users: emptyPage<User>(), provinces: [], cities: [], loading: false, error: '' }
const pageParams = ({ page, pageSize, search }: PageRequest) => ({ params: { page, pageSize, search: search || undefined } })

export const loadTenants = createAsyncThunk('data/tenants', async (request: PageRequest) => (await api.get<Page<Tenant>>('/tenant', pageParams(request))).data)
export const loadPlans = createAsyncThunk('data/plans', async (request: PageRequest) => (await api.get<Page<BusinessPlan>>('/business-plan', pageParams(request))).data)
export const loadUsers = createAsyncThunk('data/users', async (request: PageRequest) => (await api.get<Page<User>>('/user', pageParams(request))).data)
export const loadProvinces = createAsyncThunk('data/provinces', async (countryCode: string) => (await api.get<Province[]>('/province', { params: { countryCode, country_code: countryCode } })).data)
export const loadCities = createAsyncThunk('data/cities', async (provinceId: number) => (await api.get<City[]>(`/cities/by-province/${provinceId}`)).data)

const dataSlice = createSlice({
  name: 'data', initialState: initialData, reducers: {
    clearDataError: (state) => { state.error = '' },
    clearCities: (state) => { state.cities = [] },
  },
  extraReducers: (builder) => {
    const listThunks = [loadTenants, loadPlans, loadUsers, loadProvinces, loadCities] as const
    listThunks.forEach((thunk) => {
      builder.addCase(thunk.pending, (state) => { state.loading = true; state.error = '' })
      builder.addCase(thunk.rejected, (state, action) => { state.loading = false; state.error = String(action.error.message ?? '') })
    })
    builder.addCase(loadTenants.fulfilled, (s, a) => { s.loading = false; s.tenants = a.payload })
    builder.addCase(loadPlans.fulfilled, (s, a) => { s.loading = false; s.plans = a.payload })
    builder.addCase(loadUsers.fulfilled, (s, a) => { s.loading = false; s.users = a.payload })
    builder.addCase(loadProvinces.fulfilled, (s, a) => { s.loading = false; s.provinces = a.payload; s.cities = [] })
    builder.addCase(loadCities.fulfilled, (s, a) => { s.loading = false; s.cities = a.payload })
  },
})

export const store = configureStore({ reducer: { auth: authSlice.reducer, data: dataSlice.reducer } })
export const { logout, clearAuthError } = authSlice.actions
export const { clearDataError, clearCities } = dataSlice.actions
export type RootState = ReturnType<typeof store.getState>
export type AppDispatch = typeof store.dispatch
