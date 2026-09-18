import { useState, type FormEvent } from 'react'
import { Eye, EyeOff, LockKeyhole, ShieldCheck } from 'lucide-react'
import { useDispatch, useSelector } from 'react-redux'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { api, apiMessage } from '../api'
import type { AppDispatch, RootState } from '../store'
import { changePassword, login } from '../store'

export function LoginPage() {
  const { t, i18n } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const { loading, error } = useSelector((s: RootState) => s.auth)
  const [show, setShow] = useState(false); const [form, setForm] = useState({ email: '', password: '' })
  const submit = (e: FormEvent) => { e.preventDefault(); dispatch(login(form)) }
  return <div className="auth-page"><div className="auth-panel"><div className="auth-brand"><span><ShieldCheck /></span><b>hermes</b><small>BACKOFFICE</small></div><div className="auth-copy"><span className="eyebrow">{t('adminEyebrow')}</span><h1>{t('adminHeadline')}</h1><p>{t('adminIntro')}</p></div><div className="auth-orbit"><ShieldCheck /></div></div><div className="auth-form-wrap"><button className="auth-language" onClick={() => i18n.changeLanguage(i18n.language.startsWith('pt') ? 'en' : 'pt-BR')}>{i18n.language.startsWith('pt') ? 'EN' : 'PT'}</button><form className="auth-card" onSubmit={submit}><span className="auth-icon"><LockKeyhole /></span><h2>{t('loginTitle')}</h2><p>{t('loginHint')}</p><label>{t('email')}<input type="email" autoComplete="username" required value={form.email} onChange={(e) => setForm({ ...form, email: e.target.value })} /></label><label>{t('password')}<span className="password-input"><input type={show ? 'text' : 'password'} autoComplete="current-password" required value={form.password} onChange={(e) => setForm({ ...form, password: e.target.value })} /><button type="button" onClick={() => setShow(!show)}>{show ? <EyeOff /> : <Eye />}</button></span></label>{error && <div className="form-error">{t('invalidLogin')}</div>}<button className="btn primary full" disabled={loading}>{loading ? t('signingIn') : t('signIn')}</button></form></div></div>
}

export function PasswordPage({ firstAccess = false }: { firstAccess?: boolean }) {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const navigate = useNavigate(); const [form, setForm] = useState({ currentPassword: '', newPassword: '', confirm: '' }); const [error, setError] = useState(''); const [saving, setSaving] = useState(false)
  const submit = async (e: FormEvent) => { e.preventDefault(); if (form.newPassword.length < 8 || form.newPassword !== form.confirm) { setError(form.newPassword.length < 8 ? t('passwordMin') : t('passwordMismatch')); return } setSaving(true); const result = await dispatch(changePassword({ currentPassword: form.currentPassword, newPassword: form.newPassword })); setSaving(false); if (changePassword.fulfilled.match(result)) navigate('/'); else setError(String(result.payload ?? t('genericError'))) }
  return <div className={firstAccess ? 'auth-page centered' : ''}><form className="auth-card" onSubmit={submit}><span className="auth-icon"><LockKeyhole /></span><h2>{firstAccess ? t('firstLoginTitle') : t('changePassword')}</h2><p>{firstAccess ? t('firstLoginHint') : ''}</p><label>{t('currentPassword')}<input type="password" required value={form.currentPassword} onChange={(e) => setForm({ ...form, currentPassword: e.target.value })} /></label><label>{t('newPassword')}<input type="password" required value={form.newPassword} onChange={(e) => setForm({ ...form, newPassword: e.target.value })} /></label><label>{t('confirmPassword')}<input type="password" required value={form.confirm} onChange={(e) => setForm({ ...form, confirm: e.target.value })} /></label>{error && <div className="form-error">{error}</div>}<button className="btn primary full" disabled={saving}>{saving ? t('saving') : t('changePassword')}</button></form></div>
}

// EPIC-BO-04 client half of EPIC-IA-07/D-07: the page a person lands on from
// the invitation email's link. There is no session yet -- the token in the
// URL is the only credential -- so this never goes through the authenticated
// api.ts interceptor's Bearer header, just a plain POST.
export function AcceptInvitePage() {
  const { t } = useTranslation()
  const navigate = useNavigate()
  const [params] = useSearchParams()
  const token = params.get('token') || ''
  const [form, setForm] = useState({ newPassword: '', confirm: '' })
  const [error, setError] = useState('')
  const [saving, setSaving] = useState(false)
  const [done, setDone] = useState(false)

  const submit = async (e: FormEvent) => {
    e.preventDefault()
    if (form.newPassword.length < 8 || form.newPassword !== form.confirm) {
      setError(form.newPassword.length < 8 ? t('passwordMin') : t('passwordMismatch'))
      return
    }
    setSaving(true)
    setError('')
    try {
      await api.post('/accept-invite', { token, newPassword: form.newPassword })
      setDone(true)
    } catch (cause) {
      setError(apiMessage(cause, t('genericError')))
    } finally {
      setSaving(false)
    }
  }

  if (!token) return <div className="auth-page centered"><div className="auth-card"><span className="auth-icon"><LockKeyhole /></span><h2>{t('inviteInvalidTitle')}</h2><p>{t('inviteInvalidHint')}</p></div></div>

  if (done) return <div className="auth-page centered"><div className="auth-card"><span className="auth-icon"><LockKeyhole /></span><h2>{t('inviteAcceptedTitle')}</h2><p>{t('inviteAcceptedHint')}</p><button className="btn primary full" onClick={() => navigate('/login')}>{t('signIn')}</button></div></div>

  return <div className="auth-page centered"><form className="auth-card" onSubmit={submit}><span className="auth-icon"><LockKeyhole /></span><h2>{t('inviteTitle')}</h2><p>{t('inviteHint')}</p><label>{t('newPassword')}<input type="password" required value={form.newPassword} onChange={(e) => setForm({ ...form, newPassword: e.target.value })} /></label><label>{t('confirmPassword')}<input type="password" required value={form.confirm} onChange={(e) => setForm({ ...form, confirm: e.target.value })} /></label>{error && <div className="form-error">{error}</div>}<button className="btn primary full" disabled={saving}>{saving ? t('saving') : t('inviteSubmit')}</button></form></div>
}
