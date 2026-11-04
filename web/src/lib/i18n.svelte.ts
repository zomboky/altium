// The dashboard speaks English or French: the browser's language at first,
// then the one picked in the header, remembered in localStorage.
//
// Texts are written in place as `t('English', 'Français')`: both stay next to
// the markup that shows them, and reading `i18n.lang` makes them reactive.

export type Lang = 'en' | 'fr'

export const LANGS: Record<Lang, string> = { en: 'English', fr: 'Français' }

const KEY = 'altium.lang'

function initial(): Lang {
  try {
    const stored = localStorage.getItem(KEY)
    if (stored === 'en' || stored === 'fr') return stored
  } catch {
    // No storage (private window, blocked site data): follow the browser.
  }
  return typeof navigator !== 'undefined' && navigator.language.toLowerCase().startsWith('fr') ? 'fr' : 'en'
}

export const i18n = $state<{ lang: Lang }>({ lang: initial() })

if (typeof document !== 'undefined') document.documentElement.lang = i18n.lang

export function setLang(lang: Lang) {
  i18n.lang = lang
  document.documentElement.lang = lang
  try {
    localStorage.setItem(KEY, lang)
  } catch {
    // Kept for this page only.
  }
}

export function t(en: string, fr: string): string {
  return i18n.lang === 'fr' ? fr : en
}

/** The BCP 47 locale for `Intl` and `localeCompare`. */
export function locale(): string {
  return i18n.lang === 'fr' ? 'fr-FR' : 'en-US'
}

/** A label map that reads the current language on every access. */
export function labels<K extends string>(texts: Record<K, [en: string, fr: string]>): Record<K, string> {
  const label = (key: string | symbol): string | undefined => {
    const pair: [string, string] | undefined = Object.hasOwn(texts, key) ? texts[key as K] : undefined
    return pair && t(pair[0], pair[1])
  }
  // `Object.entries` and `in` see the keys too, with their current label.
  return new Proxy({} as Record<K, string>, {
    get: (_, key) => label(key),
    has: (_, key) => Object.hasOwn(texts, key),
    ownKeys: () => Reflect.ownKeys(texts),
    getOwnPropertyDescriptor: (_, key) =>
      Object.hasOwn(texts, key) ? { enumerable: true, configurable: true, value: label(key) } : undefined,
  })
}
