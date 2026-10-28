import type { FormEvent, ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

/// Cancel on the left, Save on the right. A read-only form (a system profile, say) offers only the way back.
export function Form({ children, onSubmit, error, saving, onCancel, readOnly = false }: { children: ReactNode; onSubmit: (event: FormEvent) => void; error?: string; saving?: boolean; onCancel?: () => void; readOnly?: boolean }) {
  const { t } = useTranslation()
  return <form onSubmit={onSubmit}><div className="form-grid">{children}</div>{error && <div className="form-error">{error}</div>}<div className="form-actions">{onCancel && <button type="button" className="btn secondary" onClick={onCancel}>{readOnly ? t('back') : t('cancel')}</button>}{!readOnly && <button className="btn primary" disabled={saving}>{saving ? t('saving') : t('save')}</button>}</div></form>
}
