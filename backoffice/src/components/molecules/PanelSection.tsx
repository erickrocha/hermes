import type { ReactNode } from 'react'
import { useTranslation } from 'react-i18next'

/// Um bloco de uma página de detalhe que carrega por conta própria: título, ações, e os quatro estados da leitura.
export function PanelSection({ titleKey, loading, error, empty, actions, children }: { titleKey: string; loading: boolean; error?: string; empty?: boolean; actions?: ReactNode; children: ReactNode }) {
  const { t } = useTranslation()
  return <section className="page-section">
    <div className="section-head"><h2>{t(titleKey)}</h2>{actions && <div className="inline-actions">{actions}</div>}</div>
    {loading ? <p className="form-hint">{t('loading')}</p> : error ? <p className="form-hint" role="status">{t(error, { defaultValue: error })}</p> : empty ? <p className="form-hint">{t('noResults')}</p> : children}
  </section>
}
