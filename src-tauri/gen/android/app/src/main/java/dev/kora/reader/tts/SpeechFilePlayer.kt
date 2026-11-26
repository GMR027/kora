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
 * Mantiene una playlist persistente de Media3. La primera frase crea el
 * `ExoPlayer`; las siguientes se añaden a la cola mientras suena la actual,
 * de modo que no se recrea el reproductor entre frases.
 */
object SpeechFilePlayer {
    private val handler = Handler(Looper.getMainLooper())
    private val queue = mutableListOf<String>()

    @Volatile private var state = "idle"
    @Volatile private var error: String? = null
    @Volatile private var currentIndex = 0
    @Volatile private var positionMs = 0L
    private var player: ExoPlayer? = null

    fun status(): Pair<String, String?> = state to error
    fun currentIndex(): Int = currentIndex
    fun positionMs(): Long = positionMs
    fun queueSize(): Int = queue.size

    /** Reproduce un único archivo, reiniciando cualquier cola anterior. */
    fun play(context: Context, path: String, volume: Float) {
        state = "starting"
        error = null
        handler.post {
            release()
            queue.clear()
            currentIndex = 0
            positionMs = 0
            enqueueOnMain(context, path, volume)
        }
    }

    /** Añade un WAV al final de la cola sin cortar el audio actual. */
    fun enqueue(context: Context, path: String, volume: Float) {
        if (state == "idle" || state == "finished" || state == "error") {
            state = "starting"
            error = null
        }
        handler.post { enqueueOnMain(context, path, volume) }
    }

    fun pause() {
        handler.post {
            player?.pause()
            if (state == "playing" || state == "buffering" || state == "starting") {
                state = "paused"
            }
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
            queue.clear()
            currentIndex = 0
            positionMs = 0
            state = "idle"
            error = null
        }
    }

    private fun enqueueOnMain(context: Context, path: String, volume: Float) {
        val file = File(path)
        if (!file.isFile) {
            state = "error"
            error = "el archivo de audio no existe"
            return
        }

        runCatching {
            val exo = player ?: createPlayer(context)
            exo.volume = volume.coerceIn(0f, 1f)
            exo.addMediaItem(MediaItem.fromUri(Uri.fromFile(file)))
            queue.add(path)
            // Si la cola ya había terminado, hay que volver a prepararla para
            // que el nuevo elemento empiece a sonar.
            if (exo.playbackState == Player.STATE_IDLE || exo.playbackState == Player.STATE_ENDED) {
                exo.prepare()
                exo.playWhenReady = true
            }
        }.onFailure { throwable ->
            state = "error"
            error = throwable.message ?: "no se pudo abrir el audio"
            release()
        }
    }

    private fun createPlayer(context: Context): ExoPlayer {
        val exo = ExoPlayer.Builder(context.applicationContext).build()
        exo.setAudioAttributes(
            AudioAttributes.Builder()
                .setUsage(C.USAGE_MEDIA)
                .setContentType(C.AUDIO_CONTENT_TYPE_SPEECH)
                .build(),
            true
        )
        exo.setWakeMode(C.WAKE_MODE_LOCAL)
        exo.addListener(object : Player.Listener {
            override fun onMediaItemTransition(mediaItem: MediaItem?, reason: Int) {
                currentIndex = exo.currentMediaItemIndex.coerceAtLeast(0)
                if (exo.isPlaying) state = "playing"
            }

            override fun onPlaybackStateChanged(playbackState: Int) {
                when (playbackState) {
                    Player.STATE_ENDED -> {
                        state = "finished"
                        positionMs = exo.duration.coerceAtLeast(0)
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
        player = exo
        return exo
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
