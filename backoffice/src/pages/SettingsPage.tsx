import { useTranslation } from 'react-i18next'
import { PageHeader } from '../components/UI'
import { PasswordPage } from './AuthPages'
export function SettingsPage() { const { t } = useTranslation(); return <><PageHeader title={t('settings')} /><div className="settings-card"><PasswordPage /></div></> }
