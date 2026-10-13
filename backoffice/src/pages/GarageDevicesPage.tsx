import { useCallback, useEffect, useState } from 'react'
import { CircleCheck, CircleOff, Copy, Monitor, RefreshCw, ShieldOff } from 'lucide-react'
import { yupResolver } from '@hookform/resolvers/yup'
import { useForm } from 'react-hook-form'
import * as yup from 'yup'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { Confirm, Empty, Loading, PageHeader } from '../components/UI'
import type { AppDispatch, RootState } from '../store'
import { fetchGarageDevices, issueGarageDeviceCode, reissueGarageDeviceCode, revokeGarageDevice } from '../features/garageDeviceSlice'
import type { GarageDevice, PairingCode } from '../services/garageDeviceService'

export function GarageDevicesPage() {
  const { t } = useTranslation()
  const dispatch = useDispatch<AppDispatch>()
  const { items, loading, saving, error } = useSelector((state: RootState) => state.garageDevices)
  const [pairing, setPairing] = useState<PairingCode | null>(null)
  const [copied, setCopied] = useState(false)
  const [revoking, setRevoking] = useState<GarageDevice | null>(null)
  const schema = yup.object({ name: yup.string().trim().required(t('genericError')) })
  const { register, handleSubmit, reset, formState: { errors } } = useForm<{ name: string }>({ resolver: yupResolver(schema), defaultValues: { name: '' } })

  const refresh = useCallback(() => dispatch(fetchGarageDevices()), [dispatch])
  useEffect(() => { void refresh() }, [refresh])

  const issue = handleSubmit(async ({ name }) => {
    setPairing(null)
    setCopied(false)
    try {
      const code = await dispatch(issueGarageDeviceCode(name)).unwrap()
      setPairing(code)
      reset()
      await dispatch(fetchGarageDevices()).unwrap()
    } catch { /* Redux keeps the translated API error for the page. */ }
  })

  const reissue = async (device: GarageDevice) => {
    setPairing(null)
    setCopied(false)
    try {
      setPairing(await dispatch(reissueGarageDeviceCode(device.uuid)).unwrap())
      await refresh().unwrap()
    } catch { /* Redux keeps the translated API error for the page. */ }
  }

  const revoke = async () => {
    if (!revoking) return
    try {
      await dispatch(revokeGarageDevice(revoking.uuid)).unwrap()
      setRevoking(null)
      setPairing(null)
      await refresh().unwrap()
    } catch { /* Redux keeps the translated API error for the page. */ }
  }

  const copyCode = async () => {
    if (!pairing) return
    try { await navigator.clipboard.writeText(pairing.code); setCopied(true) } catch { setCopied(false) }
  }

  return <>
    <PageHeader title={t('garageDevices')} subtitle={t('garageDevicesText')} actions={<button className="btn secondary" disabled={loading} onClick={() => void refresh()}><RefreshCw size={16} />{t('refresh')}</button>} />
    <section className="page-form-card">
      <h2>{t('newGarageDevice')}</h2>
      <form className="form-grid" onSubmit={issue} noValidate>
        <label className="span-2">{t('deviceName')}<input required maxLength={120} {...register('name')} />{errors.name?.message && <small className="form-error">{errors.name.message}</small>}</label>
        <div className="span-2 form-actions"><button className="btn primary" disabled={saving}>{saving ? t('saving') : t('issuePairingCode')}</button></div>
      </form>
    </section>
    {error && <p className="form-error" role="alert">{t(error, { defaultValue: error })}</p>}
    {pairing && <section className="device-pairing-result" role="status">
      <div><strong>{pairing.deviceName}</strong><p>{t('pairingCodeHint')}</p><p>{t('pairingExpires')}: {new Date(pairing.expiresAt).toLocaleString()}</p></div>
      <output className="device-pairing-code" aria-label={t('pairingCode')}>{pairing.code}</output>
      <button type="button" className="btn secondary" onClick={() => void copyCode()} aria-label={copied ? t('copied') : t('copyPairingCode')} title={copied ? t('copied') : t('copyPairingCode')}><Copy size={16} />{copied ? t('copied') : t('copyPairingCode')}</button>
    </section>}
    <section>
      <h2>{t('garageDevices')}</h2>
      {loading && !items.length ? <Loading /> : !items.length ? <Empty /> : <div className="table-card"><table>
        <thead><tr><th>{t('deviceName')}</th><th>{t('status')}</th><th>{t('pairedAt')}</th><th>{t('actions')}</th></tr></thead>
        <tbody>{items.map((device) => <tr key={device.uuid}>
          <td><Monitor size={16} /> {device.name}</td>
          <td><span className={`badge ${device.revoked ? 'gray' : device.paired ? 'green' : 'blue'}`}>{device.revoked ? <><CircleOff size={13} />{t('revoked')}</> : device.paired ? <><CircleCheck size={13} />{t('paired')}</> : t('notPaired')}</span></td>
          <td>{device.pairedAt ? new Date(device.pairedAt).toLocaleString() : '—'}</td>
          <td className="row-actions"><button type="button" className="btn secondary" disabled={saving} onClick={() => void reissue(device)}><RefreshCw size={15} />{t('reissueCode')}</button>{!device.revoked && <button type="button" className="danger-text" disabled={saving} onClick={() => setRevoking(device)} aria-label={t('revokeDevice')} title={t('revokeDevice')}><ShieldOff size={16} /></button>}</td>
        </tr>)}</tbody>
      </table></div>}
    </section>
    {revoking && <Confirm title={t('revokeDeviceTitle')} text={t('revokeDeviceText', { name: revoking.name })} onCancel={() => setRevoking(null)} onConfirm={revoke} />}
  </>
}