package dev.kora.reader.tts

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.Build
import android.util.Log

/**
 * Audio focus por locución. El motor TTS de Google pide su propio focus, así
 * que una petición permanente provocaría que el sistema nos lo retirase justo
 * al empezar a hablar. Se usa `AUDIOFOCUS_GAIN_TRANSIENT`, igual que el plugin
 * anterior.
 */
class TtsAudioFocus(
    context: Context,
    private val onLoss: (String) -> Unit,
) {
    private val audioManager =
        context.getSystemService(Context.AUDIO_SERVICE) as? AudioManager
    private var focusRequest: AudioFocusRequest? = null
    private var legacyListener: AudioManager.OnAudioFocusChangeListener? = null

    private val listener = AudioManager.OnAudioFocusChangeListener { change ->
        when (change) {
            AudioManager.AUDIOFOCUS_LOSS ->
                onLoss("audio_focus_loss")
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT ->
                onLoss("audio_focus_transient_loss")
            AudioManager.AUDIOFOCUS_LOSS_TRANSIENT_CAN_DUCK ->
                onLoss("audio_focus_duck")
        }
    }

    fun request(): Boolean {
        release()
        val granted = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN_TRANSIENT)
                .setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_MEDIA)
                        .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                        .build()
                )
                .setWillPauseWhenDucked(false)
                .setOnAudioFocusChangeListener(listener)
                .build()
                .also { focusRequest = it }
                .let { audioManager?.requestAudioFocus(it) }
        } else {
            legacyListener = listener
            @Suppress("DEPRECATION")
            audioManager?.requestAudioFocus(
                listener,
                AudioManager.STREAM_MUSIC,
                AudioManager.AUDIOFOCUS_GAIN_TRANSIENT,
            )
        }

        val ok = granted == AudioManager.AUDIOFOCUS_REQUEST_GRANTED
        if (!ok) {
            Log.w(TAG, "audio focus no concedido")
        }
        return ok
    }

    fun release() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            focusRequest?.let { audioManager?.abandonAudioFocusRequest(it) }
            focusRequest = null
        } else {
            legacyListener?.let {
                @Suppress("DEPRECATION")
                audioManager?.abandonAudioFocus(it)
            }
            legacyListener = null
        }
    }

    private companion object {
        const val TAG = "KoraTts"
    }
}
