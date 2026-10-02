import { Languages } from 'lucide-react'
import { useTranslation } from 'react-i18next'
import { LANGUAGES, currentLanguage } from '../i18n'

// DEF-BO-06: one control, used by both the shell topbar and the sign-in page,
// so the two can never drift apart again. It is a `select` rather than a
// toggle because three choices do not fit a toggle, and because a select
// shows the *current* language as its value -- the old button showed the one
// you would switch to, which read as a status label and confused testers.
export function LanguagePicker({ className = 'language-button' }: { className?: string }) {
  const { t, i18n } = useTranslation()
  const active = currentLanguage(i18n.language)
  return <span className={`${className} language-picker`}>
    <Languages size={18} aria-hidden="true" />
    <select aria-label={t('language')} value={active.code} onChange={(event) => i18n.changeLanguage(event.target.value)}>
      {LANGUAGES.map(({ code, label }) => <option key={code} value={code}>{label}</option>)}
    </select>
  </span>
}
