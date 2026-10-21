import { Search } from 'lucide-react'
import { useTranslation } from 'react-i18next'

export function Empty() { const { t } = useTranslation(); return <div className="state-box"><Search /><p>{t('noResults')}</p></div> }
