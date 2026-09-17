import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import LanguageDetector from 'i18next-browser-languagedetector'

import pt from './locales/pt.json'
import en from './locales/en.json'

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      pt: { translation: pt },
      'pt-BR': { translation: pt },
      en: { translation: en },
      'en-US': { translation: en },
    },
    fallbackLng: 'pt',
    supportedLngs: ['pt', 'pt-BR', 'en', 'en-US'],
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
    document.documentElement.lang = language?.startsWith('en') ? 'en-US' : 'pt-BR'
  }
}

syncDocumentLanguage(i18n.resolvedLanguage || i18n.language)
i18n.on('languageChanged', syncDocumentLanguage)

export const localeCountryCode = (language = i18n.resolvedLanguage || i18n.language) =>
  language?.toLowerCase().startsWith('pt') ? 'BR' : 'US'

export default i18n
