import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import LanguageDetector from 'i18next-browser-languagedetector'

import pt from './locales/pt.json'
import en from './locales/en.json'
import es from './locales/es.json'

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      pt: { translation: pt },
      'pt-BR': { translation: pt },
      en: { translation: en },
      'en-US': { translation: en },
      // PD-031: es sem região atende es-AR/es-MX/es-CO -- o mesmo que a
      // negociação do backend passou a fazer.
      es: { translation: es },
    },
    fallbackLng: 'pt',
    supportedLngs: ['pt', 'pt-BR', 'en', 'en-US', 'es'],
    nonExplicitSupportedLngs: true,
    interpolation: {
      escapeValue: false,
    },
    detection: {
      order: ['localStorage', 'navigator'],
      caches: ['localStorage'],
      lookupLocalStorage: 'i18nextLng',
    },
  })

const syncDocumentLanguage = (language?: string) => {
  if (typeof document !== 'undefined') {
    if (language?.startsWith('en')) document.documentElement.lang = 'en-US'
    else if (language?.startsWith('es')) document.documentElement.lang = 'es'
    else document.documentElement.lang = 'pt-BR'
  }
}

syncDocumentLanguage(i18n.resolvedLanguage || i18n.language)
i18n.on('languageChanged', syncDocumentLanguage)

// DEF-BO-06 (PD-031): the console shipped a two-way toggle labelled with the
// *other* language, so Spanish -- which is loaded right above -- could not be
// reached at all and the button read as a status, not a control. This is the
// list every language control renders, so adding a locale here is the only
// change a new language needs.
export const LANGUAGES = [
  { code: 'pt-BR', short: 'PT', label: 'Português' },
  { code: 'en', short: 'EN', label: 'English' },
  { code: 'es', short: 'ES', label: 'Español' },
] as const

export const currentLanguage = (language = i18n.resolvedLanguage || i18n.language) => {
  const prefix = language?.toLowerCase() ?? ''
  if (prefix.startsWith('en')) return LANGUAGES[1]
  if (prefix.startsWith('es')) return LANGUAGES[2]
  return LANGUAGES[0]
}

export const localeCountryCode = (language = i18n.resolvedLanguage || i18n.language) =>
  language?.toLowerCase().startsWith('pt') ? 'BR' : 'US'

export default i18n
