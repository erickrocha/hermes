import { useMemo, useState } from 'react'
import { useTranslation } from 'react-i18next'
import { grantCode, isOwnGrant, ownGrant } from '../../features/access'
import type { Grant, Permission } from '../../types'

export interface PermissionPickerProps {
  catalogue: Permission[]
  value: Grant[]
  onChange: (grants: Grant[]) => void
  readOnly?: boolean
  /** Codes left out of the picker -- what the profile already grants, when picking a user's extras. */
  exclude?: Iterable<string>
  /** Prefix for the accessible names, so two pickers on one form stay distinguishable. */
  name?: string
}

/** Permissions grouped by module, in catalogue order; the picker shows one group per module. */
/** i18n keys for a permission's label and its module -- the code stays in the tooltip for whoever needs to cite it. */
export const permissionKey = (code: string) => `perm_${code.replace(/\W/g, '_')}`
export const moduleKey = (module: string) => `module_${module.toLowerCase().replace(/\W+/g, '_')}`

export const groupByModule = (permissions: Permission[]) =>
  permissions.reduce<Record<string, Permission[]>>((groups, p) => ({ ...groups, [p.module]: [...(groups[p.module] ?? []), p] }), {})

/** The grant held for a code, if any: `code`, `code@own` or nothing. */
export const grantFor = (grants: Grant[], code: string) => grants.find((grant) => grantCode(grant) === code)

// C-035: a grant is `code` or `code@own`. The checkbox holds the permission; for a permission the catalogue marks
// `own: true`, a second toggle narrows it to the holder's own records and the picker emits `code@own` instead.
export function PermissionPicker({ catalogue, value, onChange, readOnly = false, exclude, name = 'permissions' }: PermissionPickerProps) {
  const { t } = useTranslation()
  const excluded = useMemo(() => new Set(exclude ?? []), [exclude])
  const groups = useMemo(() => groupByModule(catalogue.filter((permission) => !excluded.has(permission.code))), [catalogue, excluded])
  const replace = (code: string, next: Grant | null) => onChange([...value.filter((grant) => grantCode(grant) !== code), ...(next ? [next] : [])])
  const entries = Object.entries(groups)
  const [chosen, setChosen] = useState('')
  const active = groups[chosen] ? chosen : entries[0]?.[0]
  if (!entries.length) return <p className="form-hint span-2">{t('noPermissionsToPick')}</p>
  return <div className="permission-picker span-2" data-testid={name}>
    <div className="permission-tabs" role="tablist">
      {entries.map(([module, perms]) => {
        const count = perms.filter((p) => grantFor(value, p.code)).length
        return <button type="button" role="tab" key={module} aria-selected={module === active} className={module === active ? 'active' : ''} onClick={() => setChosen(module)}>{t(moduleKey(module))}<small>{count}/{perms.length}</small></button>
      })}
    </div>
    {entries.filter(([module]) => module === active).map(([module, perms]) => <fieldset className="permission-group" key={module} disabled={readOnly}>
      <legend>{t(moduleKey(module))}</legend>
      {perms.map((permission) => {
        const held = grantFor(value, permission.code)
        const own = held ? isOwnGrant(held) : false
        return <div className="permission-row" key={permission.code}>
          <label className="checkbox">
            <input type="checkbox" checked={Boolean(held)} onChange={() => replace(permission.code, held ? null : permission.code)} />
            <span className="permission-code" title={permission.code}>{t(permissionKey(permission.code), { defaultValue: permission.code })}</span>
          </label>
          {permission.own && held && <label className="checkbox permission-own">
            <input type="checkbox" checked={own} aria-label={`${permission.code} ${t('ownOnly')}`} onChange={() => replace(permission.code, own ? permission.code : ownGrant(permission.code))} />
            <span>{t('ownOnly')}</span>
          </label>}
          {permission.defaultFor.length > 0 && <small className="permission-defaults">{t('defaultFor')}: {permission.defaultFor.map((key) => t(`profile${key}`)).join(', ')}</small>}
        </div>
      })}
    </fieldset>)}
  </div>
}
