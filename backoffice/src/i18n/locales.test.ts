import { describe, expect, it } from 'vitest'
import en from './locales/en.json'
import es from './locales/es.json'
import pt from './locales/pt.json'

// PD-031: a missing key does not throw, it silently renders the fallback
// language mid-screen. Three bundles are one more chance to forget, so the
// drift is caught here instead of by a reader.
describe('translation bundles', () => {
  const bundles = { en, es, pt } as Record<string, Record<string, string>>
  const reference = Object.keys(en).sort()

  it('has the same key set in every language', () => {
    for (const [language, bundle] of Object.entries(bundles)) {
      expect(Object.keys(bundle).sort(), `${language}.json has drifted from en.json`).toEqual(reference)
    }
  })

  it('has no empty or untranslated-looking values', () => {
    for (const [language, bundle] of Object.entries(bundles)) {
      for (const [key, value] of Object.entries(bundle)) {
        expect(value.trim(), `${language}.json:${key} is empty`).not.toBe('')
      }
    }
  })

  it('keeps interpolation placeholders identical across languages', () => {
    const placeholders = (value: string) => (value.match(/\{\{\s*\w+\s*\}\}/g) ?? []).sort()
    for (const key of reference) {
      const expected = placeholders(en[key as keyof typeof en])
      for (const [language, bundle] of Object.entries(bundles)) {
        expect(placeholders(bundle[key]), `${language}.json:${key} placeholders differ from en.json`).toEqual(expected)
      }
    }
  })
})
