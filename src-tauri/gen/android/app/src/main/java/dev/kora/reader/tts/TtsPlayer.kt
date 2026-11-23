package dev.kora.reader.tts

import android.net.Uri
import android.os.Looper
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.PlaybackParameters
import androidx.media3.common.Player
import androidx.media3.common.SimpleBasePlayer
import androidx.media3.common.util.UnstableApi
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture

/**
 * Adaptador de solo lectura entre el estado del TTS y Media3.
 *
 * No reproduce audio: el audio lo sigue generando `TextToSpeech`. El player
 * existe para que `MediaSessionService` publique metadatos, notificación y
 * controles de pantalla de bloqueo.
 */
@UnstableApi
class TtsPlayer(
    looper: Looper,
    private val onControl: (String) -> Unit,
) : SimpleBasePlayer(looper) {

    data class Model(
        val title: String = "Kora",
        val artist: String = "Lectura en voz alta",
        val album: String? = null,
        val coverUri: Uri? = null,
        val artworkData: ByteArray? = null,
        val active: Boolean = false,
        val playing: Boolean = false,
        val speed: Float = 1f,
    )

    private var model = Model()

    fun update(next: Model) {
        model = next
        invalidateState()
    }

    fun current(): Model = model

    override fun getState(): State {
        val metadata = buildMetadata()
        val item = MediaItem.Builder()
            .setMediaId(MEDIA_ID)
            .setMediaMetadata(metadata)
            .build()
        val itemData = MediaItemData.Builder(MEDIA_ID)
            .setMediaItem(item)
            .setMediaMetadata(metadata)
            .setDurationUs(C.TIME_UNSET)
            .setIsSeekable(false)
            .build()

        val commands = Player.Commands.Builder()
            .addAllReadOnlyCommands()
            .add(Player.COMMAND_PREPARE)
            .add(Player.COMMAND_PLAY_PAUSE)
            .add(Player.COMMAND_STOP)
            .build()

        return State.Builder()
            .setAvailableCommands(commands)
            .setPlayWhenReady(
                model.playing,
                Player.PLAY_WHEN_READY_CHANGE_REASON_USER_REQUEST,
            )
            .setPlaybackState(if (model.active) Player.STATE_READY else Player.STATE_IDLE)
            .setPlaybackParameters(PlaybackParameters(model.speed.coerceIn(0.5f, 3f)))
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(C.USAGE_MEDIA)
                    .setContentType(C.AUDIO_CONTENT_TYPE_SPEECH)
                    .build()
            )
            .setPlaylist(listOf(itemData))
            .setCurrentMediaItemIndex(0)
            .build()
    }

    override fun handleSetPlayWhenReady(playWhenReady: Boolean): ListenableFuture<*> {
        if (!model.active || model.playing == playWhenReady) {
            return Futures.immediateVoidFuture()
        }
        model = model.copy(playing = playWhenReady)
        onControl(if (playWhenReady) "play" else "pause")
        invalidateState()
        return Futures.immediateVoidFuture()
    }

    override fun handleStop(): ListenableFuture<*> {
        if (!model.active) {
            return Futures.immediateVoidFuture()
        }
        model = model.copy(active = false, playing = false)
        onControl("stop")
        invalidateState()
        return Futures.immediateVoidFuture()
    }

    override fun handlePrepare(): ListenableFuture<*> = Futures.immediateVoidFuture()

    override fun handleRelease(): ListenableFuture<*> = Futures.immediateVoidFuture()

    private fun buildMetadata(): MediaMetadata {
        val builder = MediaMetadata.Builder()
            .setTitle(model.title)
            .setArtist(model.artist)
            .setAlbumTitle(model.album?.takeIf { it.isNotBlank() } ?: "Kora")
            .setDisplayTitle(model.title)
            .setMediaType(MediaMetadata.MEDIA_TYPE_AUDIO_BOOK)
            .setIsPlayable(true)
            .setIsBrowsable(false)

        when {
            model.coverUri != null -> builder.setArtworkUri(model.coverUri)
            else -> model.artworkData?.let {
                builder.setArtworkData(it, MediaMetadata.PICTURE_TYPE_FRONT_COVER)
            }
        }
        return builder.build()
    }

    private companion object {
        const val MEDIA_ID = "kora-tts"
    }
}
