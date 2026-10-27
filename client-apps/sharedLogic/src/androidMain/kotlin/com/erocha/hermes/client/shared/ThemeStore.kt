package com.erocha.hermes.client.shared

import android.content.Context
import android.util.TypedValue

/** The brand identities both apps can wear; mirrors `ThemeName` in the console's `theme.ts`. */
enum class HermesTheme {
    TRANSMEGA,
    HERMES;

    fun next(): HermesTheme = entries[(ordinal + 1) % entries.size]
}

/** Remembers the chosen [HermesTheme] on the device (a display preference, so plain preferences). */
class ThemeStore(context: Context) {
    private val preferences = context.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)

    fun load(): HermesTheme =
        HermesTheme.entries.firstOrNull { it.name == preferences.getString(KEY, null) } ?: HermesTheme.TRANSMEGA

    fun save(theme: HermesTheme) = preferences.edit().putString(KEY, theme.name).apply()

    private companion object {
        const val PREFERENCES = "hermes_theme"
        const val KEY = "theme"
    }
}

/** Resolves a colour theme attribute (`?attr/...`) against this context's current theme. */
fun Context.themeColor(attr: Int): Int {
    val value = TypedValue()
    check(theme.resolveAttribute(attr, value, true)) { "Theme attribute $attr is not set" }
    return value.data
}
