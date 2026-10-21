import { Link } from 'react-router-dom'
import { useTranslation } from 'react-i18next'
import { PageHeader } from '../components/UI'

export function NotFoundPage() {
  const { t } = useTranslation()
  return <><PageHeader title={t('notFoundTitle')} subtitle={t('notFoundText')} actions={<Link className="btn primary" to="/home">{t('dashboard')}</Link>} /><p className="form-hint">404</p></>
}
