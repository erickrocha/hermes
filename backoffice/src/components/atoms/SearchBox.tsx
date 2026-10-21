import { Search } from 'lucide-react'
import { useTranslation } from 'react-i18next'

export function SearchBox({ value, onChange }: { value: string; onChange: (value: string) => void }) { const { t } = useTranslation(); return <label className="search-box"><Search size={17} /><input value={value} onChange={(e) => onChange(e.target.value)} placeholder={t('search')} /></label> }
