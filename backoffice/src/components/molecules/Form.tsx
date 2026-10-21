import type { FormEvent, ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

export function Form({ children, onSubmit, error, saving, onCancel }: { children: ReactNode; onSubmit: (event: FormEvent) => void; error?: string; saving?: boolean; onCancel?: () => void }) { const { t } = useTranslation(); return <form onSubmit={onSubmit}><div className="form-grid">{children}</div>{error && <div className="form-error">{error}</div>}<div className="form-actions">{onCancel && <button type="button" className="btn secondary" onClick={onCancel}>{t('cancel')}</button>}<button className="btn primary" disabled={saving}>{saving ? t('saving') : t('save')}</button></div></form> }
