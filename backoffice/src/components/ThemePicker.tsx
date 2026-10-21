import { useState } from 'react'
import { Palette } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { THEME_NAMES, getStoredTheme, setTheme, type ThemeName } from '../theme'

const LABEL_KEY: Record<ThemeName, string> = { default: 'themeDefault', transmega: 'themeTransmega' }

export function ThemePicker({ className = 'language-button' }: { className?: string }) {
  const { t } = useTranslation()
  const [active, setActive] = useState<ThemeName>(getStoredTheme)
  const change = (name: ThemeName) => { setTheme(name); setActive(name) }
  return <span className={`${className} language-picker`}>
    <Palette size={18} aria-hidden="true" />
    <select aria-label={t('theme')} value={active} onChange={(event) => change(event.target.value as ThemeName)}>
      {THEME_NAMES.map((name) => <option key={name} value={name}>{t(LABEL_KEY[name])}</option>)}
    </select>
  </span>
}
