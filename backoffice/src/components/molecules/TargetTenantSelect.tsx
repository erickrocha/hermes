import { useEffect, useState } from 'react'
import { Building } from 'lucide-react'
import { useDispatch, useSelector } from 'react-redux'
import { useTranslation } from 'react-i18next'
import { fetchTenants } from '../../services/dataService'
import { selectTargetTenant } from '../../store'
import type { AppDispatch, RootState } from '../../store'
import type { Tenant } from '../../types'

const OWN = ''

/// C-036 (`DS-1`): the tenant the console acts in. Offered to whoever may act inside a descendant; choosing one
/// makes every request name it (`X-Tenant-Uuid`), choosing the first option brings the console back to the signed-in tenant.
export function TargetTenantSelect() {
  const { t } = useTranslation()
  const dispatch = useDispatch<AppDispatch>()
  const session = useSelector((state: RootState) => state.auth.session)
  const target = useSelector((state: RootState) => state.auth.target)
  const [tenants, setTenants] = useState<Tenant[]>([])
  const [failed, setFailed] = useState(false)
  useEffect(() => {
    if (!session?.canTarget) return
    // The branch below the signed-in tenant: acting in a tenant is only offered where the server would allow it.
    fetchTenants({ page: 0, pageSize: 100 })
      .then((page) => { setFailed(!Array.isArray(page?.items)); setTenants(Array.isArray(page?.items) ? page.items : []) })
      .catch(() => setFailed(true))
  }, [session?.canTarget, session?.userId])
  if (!session?.canTarget) return null
  const choices = tenants.filter((tenant) => tenant.uuid && tenant.uuid !== session.tenantUuid)
  const choose = (uuid: string) => {
    const tenant = choices.find((item) => item.uuid === uuid)
    void dispatch(selectTargetTenant(tenant ? { uuid, name: tenant.companyName || tenant.businessName } : null))
  }
  return <label className="target-tenant">
    <Building size={16} aria-hidden="true" />
    <span className="sr-only">{t('actingIn')}</span>
    <select aria-label={t('actingIn')} value={target?.uuid ?? OWN} onChange={(event) => choose(event.target.value)} disabled={failed && !choices.length}>
      <option value={OWN}>{t('ownTenant')}</option>
      {choices.map((tenant) => <option key={tenant.uuid} value={tenant.uuid}>{tenant.companyName || tenant.businessName}</option>)}
    </select>
  </label>
}
