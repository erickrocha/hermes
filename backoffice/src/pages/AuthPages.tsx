import { useState } from 'react'
import { Eye, EyeOff, LockKeyhole, ShieldCheck } from 'lucide-react'
import { yupResolver } from '@hookform/resolvers/yup'
import { useForm } from 'react-hook-form'
import * as yup from 'yup'
import { useDispatch, useSelector } from 'react-redux'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import type { AppDispatch, RootState } from '../store'
import { acceptInvitation, changePassword, login } from '../store'
import { LanguagePicker } from '../components/LanguagePicker'

export function LoginPage() {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const { loading, error } = useSelector((s: RootState) => s.auth)
  const [show, setShow] = useState(false)
  const schema = yup.object({ email: yup.string().email(t('invalidLogin')).required(t('invalidLogin')), password: yup.string().required(t('invalidLogin')) })
  const { register, handleSubmit, formState: { errors: validationErrors } } = useForm({ resolver: yupResolver(schema) })
  const submit = handleSubmit((values) => { dispatch(login(values)) })
  return <div className="auth-page"><div className="auth-panel"><div className="auth-brand"><span><ShieldCheck /></span><b>hermes</b><small>BACKOFFICE</small></div><div className="auth-copy"><span className="eyebrow">{t('adminEyebrow')}</span><h1>{t('adminHeadline')}</h1><p>{t('adminIntro')}</p></div><div className="auth-orbit"><ShieldCheck /></div></div><div className="auth-form-wrap"><LanguagePicker className="auth-language" /><form className="auth-card" onSubmit={submit} noValidate><span className="auth-icon"><LockKeyhole /></span><h2>{t('loginTitle')}</h2><p>{t('loginHint')}</p><label>{t('email')}<input type="email" autoComplete="username" {...register('email')} /></label><label>{t('password')}<span className="password-input"><input type={show ? 'text' : 'password'} autoComplete="current-password" {...register('password')} /><button type="button" onClick={() => setShow(!show)}>{show ? <EyeOff /> : <Eye />}</button></span></label>{Object.values(validationErrors).map((fieldError) => fieldError?.message && <div className="form-error" key={fieldError.message}>{fieldError.message}</div>)}{error && <div className="form-error">{t('invalidLogin')}</div>}<button className="btn primary full" disabled={loading}>{loading ? t('signingIn') : t('signIn')}</button></form></div></div>
}

export function PasswordPage() {
  const { t } = useTranslation(); const dispatch = useDispatch<AppDispatch>(); const navigate = useNavigate(); const [error, setError] = useState(''); const [saving, setSaving] = useState(false)
  const schema = yup.object({ currentPassword: yup.string().required(t('genericError')), newPassword: yup.string().min(8, t('passwordMin')).required(t('genericError')), confirm: yup.string().oneOf([yup.ref('newPassword')], t('passwordMismatch')).required(t('genericError')) })
  const { register, handleSubmit, formState: { errors: validationErrors } } = useForm({ resolver: yupResolver(schema) })
  const submit = handleSubmit(async ({ currentPassword, newPassword }) => { setSaving(true); setError(''); try { await dispatch(changePassword({ currentPassword, newPassword })).unwrap(); navigate('/') } catch (cause) { setError(String(cause || t('genericError'))) } finally { setSaving(false) } })
  return <div><form className="auth-card" onSubmit={submit} noValidate><span className="auth-icon"><LockKeyhole /></span><h2>{t('changePassword')}</h2><label>{t('currentPassword')}<input type="password" {...register('currentPassword')} /></label><label>{t('newPassword')}<input type="password" {...register('newPassword')} /></label><label>{t('confirmPassword')}<input type="password" {...register('confirm')} /></label>{Object.values(validationErrors).map((fieldError) => fieldError?.message && <div className="form-error" key={fieldError.message}>{fieldError.message}</div>)}{error && <div className="form-error">{error}</div>}<button className="btn primary full" disabled={saving}>{saving ? t('saving') : t('changePassword')}</button></form></div>
}

// EPIC-BO-04 client half of EPIC-IA-07/D-07: the page a person lands on from
// the invitation email's link. There is no session yet -- the token in the
// URL is the only credential -- so this never goes through the authenticated
// api.ts interceptor's Bearer header, just a plain POST.
export function AcceptInvitePage() {
  const { t } = useTranslation()
  const dispatch = useDispatch<AppDispatch>()
  const navigate = useNavigate()
  const [params] = useSearchParams()
  const token = params.get('token') || ''
  const [error, setError] = useState('')
  const [saving, setSaving] = useState(false)
  const [done, setDone] = useState(false)
  const schema = yup.object({ newPassword: yup.string().min(8, t('passwordMin')).required(t('genericError')), confirm: yup.string().oneOf([yup.ref('newPassword')], t('passwordMismatch')).required(t('genericError')) })
  const { register, handleSubmit, formState: { errors: validationErrors } } = useForm({ resolver: yupResolver(schema) })

  const submit = handleSubmit(async ({ newPassword }) => {
    setSaving(true)
    setError('')
    try {
      await dispatch(acceptInvitation({ token, newPassword })).unwrap()
      setDone(true)
    } catch (cause) {
      setError(String(cause || t('genericError')))
    } finally {
      setSaving(false)
    }
  })

  if (!token) return <div className="auth-page centered"><div className="auth-card"><span className="auth-icon"><LockKeyhole /></span><h2>{t('inviteInvalidTitle')}</h2><p>{t('inviteInvalidHint')}</p></div></div>

  if (done) return <div className="auth-page centered"><div className="auth-card"><span className="auth-icon"><LockKeyhole /></span><h2>{t('inviteAcceptedTitle')}</h2><p>{t('inviteAcceptedHint')}</p><button className="btn primary full" onClick={() => navigate('/login')}>{t('signIn')}</button></div></div>

  return <div className="auth-page centered"><form className="auth-card" onSubmit={submit} noValidate><span className="auth-icon"><LockKeyhole /></span><h2>{t('inviteTitle')}</h2><p>{t('inviteHint')}</p><label>{t('newPassword')}<input type="password" {...register('newPassword')} /></label><label>{t('confirmPassword')}<input type="password" {...register('confirm')} /></label>{Object.values(validationErrors).map((fieldError) => fieldError?.message && <div className="form-error" key={fieldError.message}>{fieldError.message}</div>)}{error && <div className="form-error">{error}</div>}<button className="btn primary full" disabled={saving}>{saving ? t('saving') : t('inviteSubmit')}</button></form></div>
}
