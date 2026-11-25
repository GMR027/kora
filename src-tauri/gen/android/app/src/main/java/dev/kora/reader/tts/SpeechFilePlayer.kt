package dev.kora.reader.tts

import android.content.Context
import android.net.Uri
import android.os.Handler
import android.os.Looper
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import java.io.File

/**
 * Reproductor de archivos WAV para el motor Supertonic.
 *
 * Android `TextToSpeech` no acepta PCM en memoria, así que Rust sintetiza,
 * escribe un WAV temporal y este reproductor lo reproduce con Media3. El
 * estado se publica con volatilidad para que el hilo de Rust pueda consultarlo
 * sin bloquear el hilo principal.
 */
object SpeechFilePlayer {
    private val handler = Handler(Looper.getMainLooper())

    @Volatile private var state = "idle"
    @Volatile private var error: String? = null
    private var player: ExoPlayer? = null

    fun status(): Pair<String, String?> = state to error

    fun play(context: Context, path: String, volume: Float) {
        // Se marca antes de saltar al hilo principal para que el sondeo de
        // Rust no confunda "idle" con una reproducción terminada.
        state = "starting"
        error = null
        handler.post {
            release()

            val file = File(path)
            if (!file.isFile) {
                state = "error"
                error = "el archivo de audio no existe"
                return@post
            }

            runCatching {
                val exo = ExoPlayer.Builder(context.applicationContext).build()
                exo.setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(C.USAGE_MEDIA)
                        .setContentType(C.AUDIO_CONTENT_TYPE_SPEECH)
                        .build(),
                    true
                )
                exo.setWakeMode(C.WAKE_MODE_LOCAL)
                exo.volume = volume.coerceIn(0f, 1f)
                exo.addListener(object : Player.Listener {
                    override fun onPlaybackStateChanged(playbackState: Int) {
                        when (playbackState) {
                            Player.STATE_ENDED -> {
                                state = "finished"
                                release()
                            }
                            Player.STATE_READY -> {
                                if (state != "paused") state = "playing"
                            }
                            Player.STATE_BUFFERING -> state = "buffering"
                            else -> Unit
                        }
                    }

                    override fun onPlayerError(playbackError: PlaybackException) {
                        state = "error"
                        error = playbackError.message ?: "error de reproducción"
                        release()
                    }
                })
                exo.setMediaItem(MediaItem.fromUri(Uri.fromFile(file)))
                exo.prepare()
                exo.playWhenReady = true
                player = exo
            }.onFailure { throwable ->
                state = "error"
                error = throwable.message ?: "no se pudo abrir el audio"
                release()
            }
        }
    }

    fun pause() {
        handler.post {
            player?.pause()
            if (state == "playing" || state == "buffering") state = "paused"
        }
    }

    fun resume() {
        handler.post {
            player?.play()
            if (state == "paused") state = "playing"
        }
    }

    fun stop() {
        handler.post {
            release()
            state = "idle"
            error = null
        }
    }

    private fun release() {
        val current = player
        player = null
        if (current != null) {
            runCatching { current.stop() }
            runCatching { current.release() }
        }
    }
}
