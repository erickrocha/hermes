import { LoaderCircle } from 'lucide-react'
import { useTranslation } from 'react-i18next'

export function Loading() { const { t } = useTranslation(); return <div className="state-box"><LoaderCircle className="spin" /><p>{t('loading')}</p></div> }
