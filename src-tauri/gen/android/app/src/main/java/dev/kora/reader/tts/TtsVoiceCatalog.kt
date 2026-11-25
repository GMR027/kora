package dev.kora.reader.tts

import android.speech.tts.TextToSpeech
import android.speech.tts.Voice

/**
 * Voz instalada, desacoplada de [android.speech.tts.Voice] para poder filtrar y
 * ordenar el catálogo sin tocar el motor.
 *
 * Puerto del catálogo de `tauri-plugin-tts` 0.2.1 (MIT), conservando el mismo
 * identificador (`voice.name`) que ya guardan las preferencias de Kora.
 */
data class TtsVoiceInfo(
    val id: String,
    val languageTag: String,
    val displayLanguage: String,
    val displayCountry: String,
    val requiresNetwork: Boolean,
    val languageAvailable: Boolean,
    val quality: Int = 0,
    val latency: Int = 0,
    val features: Set<String> = emptySet(),
) {
    fun isUsable(): Boolean = when {
        requiresNetwork -> true
        !languageAvailable -> false
        features.contains(TextToSpeech.Engine.KEY_FEATURE_NOT_INSTALLED) -> false
        id.endsWith("-language") -> false
        else -> true
    }

    fun displayName(): String =
        listOfNotNull(localeLabel(), variantLabel(), connectionLabel()).joinToString(" · ")

    private fun localeLabel(): String =
        if (displayCountry.isEmpty()) displayLanguage else "$displayLanguage ($displayCountry)"

    private fun connectionLabel(): String = if (requiresNetwork) "Red" else "Local"

    private fun variantLabel(): String? =
        id.removeSuffixIgnoringCase("-local")
            .removeSuffixIgnoringCase("-network")
            .removePrefixIgnoringCase(languageTag)
            .removePrefix("-")
            .removePrefixIgnoringCase("x-")
            .replace('#', ' ')
            .replace('_', ' ')
            .split(" ")
            .filter { it.isNotBlank() }
            .joinToString(" ") { word ->
                if (word == word.lowercase()) word.replaceFirstChar(Char::uppercaseChar) else word
            }
            .ifBlank { null }
}

private fun String.removePrefixIgnoringCase(prefix: String): String =
    if (startsWith(prefix, ignoreCase = true)) substring(prefix.length) else this

private fun String.removeSuffixIgnoringCase(suffix: String): String =
    if (endsWith(suffix, ignoreCase = true)) dropLast(suffix.length) else this

object TtsVoiceCatalog {
    fun from(voice: Voice, engine: TextToSpeech): TtsVoiceInfo = TtsVoiceInfo(
        id = voice.name,
        languageTag = voice.locale.toLanguageTag(),
        displayLanguage = voice.locale.displayLanguage,
        displayCountry = voice.locale.country.takeIf { it.isNotEmpty() }
            ?.let { voice.locale.displayCountry }
            .orEmpty(),
        requiresNetwork = voice.isNetworkConnectionRequired,
        languageAvailable = engine.isLanguageAvailable(voice.locale) >= TextToSpeech.LANG_AVAILABLE,
        quality = voice.quality,
        latency = voice.latency,
        features = voice.features.orEmpty(),
    )

    fun languageMatches(languageTag: String, filter: String): Boolean =
        languageTag.lowercase().startsWith(filter.lowercase())

    fun listable(voices: Collection<TtsVoiceInfo>, filter: String? = null): List<TtsVoiceInfo> =
        voices
            .filter { it.isUsable() }
            .filter { filter == null || languageMatches(it.languageTag, filter) }
            .sortedWith(
                compareBy(
                    { if (it.requiresNetwork) 1 else 0 },
                    { it.languageTag },
                    { it.id },
                )
            )
            .distinctBy { it.id }

    fun fallbackFor(voices: Collection<TtsVoiceInfo>, missingId: String): TtsVoiceInfo? {
        val parts = missingId.split("-")
        val prefix = if (parts.size >= 2) "${parts[0]}-${parts[1]}" else parts[0]
        return voices.firstOrNull {
            !it.requiresNetwork && languageMatches(it.languageTag, prefix)
        }
    }

    fun defaultVoice(voices: Collection<TtsVoiceInfo>): TtsVoiceInfo? =
        voices.filter { !it.requiresNetwork }.minByOrNull { it.languageTag }
}
