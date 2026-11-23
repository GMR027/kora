package dev.kora.reader.audio

import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.MediaSession
import app.tauri.plugin.Channel
import app.tauri.plugin.JSObject
import org.json.JSONArray
import java.io.File

private const val TAG = "KoraAudio"
private const val PREFS_NAME = "audiobook_playback"
private const val KEY_AUDIOBOOK_ID = "audiobook_id"
private const val KEY_POSITION_MS = "position_ms"
private const val KEY_DURATION_MS = "duration_ms"
private const val KEY_UPDATED_AT_MS = "updated_at_ms"
private const val KEY_STATUS = "status"
private const val KEY_SRC = "src"
private const val KEY_TITLE = "title"
private const val KEY_ARTIST = "artist"
private const val KEY_COVER_PATH = "cover_path"
private const val KEY_CHAPTERS_JSON = "chapters_json"
private const val KEY_SPEED = "speed"
private const val KEY_KEEP_SCREEN_ON = "keep_screen_on"
private const val SEEK_INCREMENT_MS = 15_000L
private const val TICK_INTERVAL_MS = 1_000L
private const val CHECKPOINT_INTERVAL_MS = 5_000L

data class PlaybackChapter(
    val startMs: Long,
    val title: String,
)

data class PlaybackCheckpoint(
    val audiobookId: String,
    val positionMs: Long,
    val durationMs: Long,
    val updatedAtMs: Long,
    val status: String?,
    val src: String?,
    val title: String?,
    val artist: String?,
    val coverPath: String?,
    val chaptersJson: String?,
    val speed: Float,
    val keepScreenOn: Boolean,
)

data class PlaybackState(
    val event: String,
    val audiobookId: String?,
    val positionMs: Long,
    val durationMs: Long,
    val playing: Boolean,
    val ended: Boolean,
    val speed: Float,
    val chapterIndex: Int,
    val chapterTitle: String?,
    val sleepRemainingMs: Long,
    val error: String?,
) {
    fun toJsObject(): JSObject = JSObject().apply {
        put("event", event)
        if (audiobookId != null) put("audiobookId", audiobookId)
        put("positionMs", positionMs)
        put("durationMs", durationMs)
        put("playing", playing)
        put("ended", ended)
        put("speed", speed.toDouble())
        put("chapterIndex", chapterIndex)
        if (chapterTitle != null) put("chapterTitle", chapterTitle)
        put("sleepRemainingMs", sleepRemainingMs)
        if (error != null) put("error", error)
    }
}

/**
 * Estado de reproducción compartido por el plugin Tauri y el servicio en primer
 * plano. ExoPlayer y MediaSession viven aquí para sobrevivir a la recreación de
 * la Activity; el servicio solo aporta el ciclo de vida y la notificación.
 */
object PlaybackRuntime {
    private val lock = Any()
    private val tickHandler = Handler(Looper.getMainLooper())
    private val sleepHandler = Handler(Looper.getMainLooper())

    private var context: Context? = null
    private var player: ExoPlayer? = null
    private var sessionPlayer: ForwardingPlayer? = null
    private var mediaSession: MediaSession? = null

    private var currentId: String? = null
    private var title: String? = null
    private var artist: String? = null
    private var src: String? = null
    private var coverPath: String? = null
    private var chaptersJson: String? = null
    private var chapters: List<PlaybackChapter> = emptyList()
    private var lastError: String? = null
    private var ended = false
    private var keepScreenOn = false
    private var speed = 1f
    private var eventChannel: Channel? = null

    private var tickScheduled = false
    private var lastCheckpointAt = 0L
    private var sleepDeadlineElapsed = 0L
    private var sleepTimer: Runnable? = null

    private val ticker = object : Runnable {
        override fun run() {
            synchronized(lock) {
                val exo = player
                if (exo == null || !exo.playWhenReady || exo.playbackState == Player.STATE_ENDED) {
                    tickScheduled = false
                    return
                }
                emit("tick")
                saveCheckpoint(force = false)
                tickHandler.postDelayed(this, TICK_INTERVAL_MS)
            }
        }
    }

    private val playerListener = object : Player.Listener {
        override fun onPlaybackStateChanged(playbackState: Int) {
            synchronized(lock) {
                if (playbackState == Player.STATE_ENDED) {
                    ended = true
                    lastCheckpointAt = 0L
                    saveCheckpoint(force = true)
                }
            }
            syncTicker()
            val kind = when (playbackState) {
                Player.STATE_ENDED -> "ended"
                Player.STATE_READY -> "ready"
                Player.STATE_BUFFERING -> "loading"
                else -> "tick"
            }
            emit(kind)
        }

        override fun onIsPlayingChanged(isPlaying: Boolean) {
            syncTicker()
            emit(if (isPlaying) "play" else "pause")
            if (!isPlaying) {
                synchronized(lock) { saveCheckpoint(force = true) }
            }
        }

        override fun onPositionDiscontinuity(
            oldPosition: Player.PositionInfo,
            newPosition: Player.PositionInfo,
            reason: Int,
        ) {
            syncTicker()
            if (
                reason == Player.DISCONTINUITY_REASON_SEEK ||
                reason == Player.DISCONTINUITY_REASON_SEEK_ADJUSTMENT
            ) {
                synchronized(lock) { saveCheckpoint(force = true) }
                emit("seek")
            }
        }

        override fun onPlaybackParametersChanged(playbackParameters: androidx.media3.common.PlaybackParameters) {
            synchronized(lock) { speed = playbackParameters.speed }
            emit("speed")
        }

        override fun onPlayerError(error: PlaybackException) {
            Log.e(TAG, "error ${error.errorCodeName}: ${error.message}", error)
            synchronized(lock) {
                lastError = error.message ?: error.errorCodeName
                ended = false
                saveCheckpoint(force = true)
            }
            emit("error")
        }
    }

    fun ensure(context: Context): ExoPlayer {
        synchronized(lock) {
            player?.let { return it }
            val appContext = context.applicationContext
            this.context = appContext

            val exo = ExoPlayer.Builder(appContext)
                .setSeekBackIncrementMs(SEEK_INCREMENT_MS)
                .setSeekForwardIncrementMs(SEEK_INCREMENT_MS)
                .build()
            exo.setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(C.USAGE_MEDIA)
                    .setContentType(C.AUDIO_CONTENT_TYPE_SPEECH)
                    .build(),
                true,
            )
            exo.setHandleAudioBecomingNoisy(true)
            exo.setWakeMode(C.WAKE_MODE_LOCAL)
            exo.addListener(playerListener)
            player = exo

            val wrapper = object : ForwardingPlayer(exo) {
                override fun getAvailableCommands(): Player.Commands =
                    super.getAvailableCommands()
                        .buildUpon()
                        .add(Player.COMMAND_SEEK_BACK)
                        .add(Player.COMMAND_SEEK_FORWARD)
                        .add(Player.COMMAND_SEEK_TO_PREVIOUS)
                        .add(Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM)
                        .add(Player.COMMAND_SEEK_TO_NEXT)
                        .add(Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM)
                        .build()

                override fun isCommandAvailable(command: Int): Boolean {
                    if (command == Player.COMMAND_SEEK_TO_PREVIOUS ||
                        command == Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM ||
                        command == Player.COMMAND_SEEK_TO_NEXT ||
                        command == Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM
                    ) {
                        return chapters.size > 1
                    }
                    return super.isCommandAvailable(command)
                }

                override fun hasPreviousMediaItem(): Boolean = chapters.size > 1

                override fun hasNextMediaItem(): Boolean = chapters.size > 1

                override fun seekToPrevious() {
                    seekChapter(-1)
                }

                override fun seekToPreviousMediaItem() {
                    seekChapter(-1)
                }

                override fun seekToNext() {
                    seekChapter(1)
                }

                override fun seekToNextMediaItem() {
                    seekChapter(1)
                }
            }
            sessionPlayer = wrapper

            val launchIntent = appContext.packageManager.getLaunchIntentForPackage(appContext.packageName)
            val pendingIntent = launchIntent?.let {
                val flags = android.app.PendingIntent.FLAG_UPDATE_CURRENT or
                    android.app.PendingIntent.FLAG_IMMUTABLE
                android.app.PendingIntent.getActivity(appContext, 0, it, flags)
            }
            mediaSession = MediaSession.Builder(appContext, wrapper)
                .setPeriodicPositionUpdateEnabled(true)
                .apply {
                    if (pendingIntent != null) setSessionActivity(pendingIntent)
                }
                .build()

            return exo
        }
    }

    fun setEventChannel(channel: Channel?) {
        synchronized(lock) { eventChannel = channel }
    }

    fun play(context: Context, args: PlayArgs): PlaybackState {
        val state: PlaybackState
        synchronized(lock) {
            val exo = ensure(context)
            chapters = parseChapters(args.chaptersJson)
            chaptersJson = args.chaptersJson
            currentId = args.audiobookId
            title = args.title
            artist = args.artist
            src = args.src
            coverPath = args.coverPath
            speed = args.speed.coerceIn(0.5f, 2.5f)
            keepScreenOn = args.keepScreenOn
            ended = false
            lastError = null
            lastCheckpointAt = 0L

            val metadata = MediaMetadata.Builder()
                .setTitle(args.title.ifBlank { "Audiolibro" })
                .setArtist(args.artist?.takeIf { it.isNotBlank() } ?: "Kora")
                .setArtworkUri(args.coverPath?.let { Uri.fromFile(File(it)) })
                .setIsPlayable(true)
                .setIsBrowsable(false)
                .build()
            val itemBuilder = MediaItem.Builder()
                .setUri(Uri.fromFile(File(args.src)))
                .setMediaMetadata(metadata)
            mimeTypeFor(args.src)?.let(itemBuilder::setMimeType)
            val item = itemBuilder.build()

            startService(context)
            exo.setMediaItem(item, args.startMs.coerceAtLeast(0))
            exo.setPlaybackSpeed(speed)
            exo.prepare()
            exo.playWhenReady = true
            exo.play()

            saveCheckpoint(force = true)
            syncTickerLocked()
            state = stateLocked("play")
        }
        emit("play")
        return state
    }

    fun resume(context: Context): PlaybackState {
        synchronized(lock) {
            val exo = ensure(context)
            if (exo.mediaItemCount == 0) {
                return stateLocked("pause")
            }
            lastError = null
            if (exo.playbackState == Player.STATE_ENDED) {
                exo.seekTo(0)
            }
            ended = false
            startService(context)
            exo.playWhenReady = true
            exo.play()
            syncTickerLocked()
        }
        emit("play")
        return getState(context)
    }

    fun pause(context: Context): PlaybackState {
        synchronized(lock) {
            player?.let {
                it.playWhenReady = false
                it.pause()
            }
            syncTickerLocked()
            saveCheckpoint(force = true)
        }
        emit("pause")
        return getState(context)
    }

    fun seek(context: Context, positionMs: Long): PlaybackState {
        synchronized(lock) {
            val exo = ensure(context)
            val duration = exo.duration.takeIf { it > 0 } ?: Long.MAX_VALUE
            ended = false
            lastError = null
            exo.seekTo(positionMs.coerceIn(0, duration))
            lastCheckpointAt = 0L
            syncTickerLocked()
        }
        emit("seek")
        return getState(context)
    }

    fun setSpeed(context: Context, value: Float): PlaybackState {
        synchronized(lock) {
            val exo = ensure(context)
            speed = value.coerceIn(0.5f, 2.5f)
            exo.setPlaybackSpeed(speed)
            saveCheckpoint(force = true)
        }
        emit("speed")
        return getState(context)
    }

    fun stop(context: Context): PlaybackState {
        val snapshot: PlaybackState
        synchronized(lock) {
            saveCheckpoint(force = true)
            snapshot = stateLocked("stop")
            currentId = null
            title = null
            artist = null
            src = null
            coverPath = null
            chaptersJson = null
            chapters = emptyList()
            ended = false
            lastError = null
            tickHandler.removeCallbacks(ticker)
            tickScheduled = false
            cancelSleepTimerLocked()
            player?.let {
                it.playWhenReady = false
                it.stop()
                it.clearMediaItems()
            }
        }
        emit("stop")
        context.stopService(Intent(context, AudiobookPlaybackService::class.java))
        return snapshot
    }

    fun getState(context: Context): PlaybackState {
        @Suppress("UNUSED_PARAMETER")
        val unused = context
        synchronized(lock) {
            return stateLocked("state")
        }
    }

    fun getCheckpoint(context: Context): PlaybackCheckpoint? {
        val prefs = prefs(context)
        val id = prefs.getString(KEY_AUDIOBOOK_ID, null)?.trim().orEmpty()
        if (id.isEmpty()) return null
        val position = prefs.getLong(KEY_POSITION_MS, 0L)
        val updatedAt = prefs.getLong(KEY_UPDATED_AT_MS, 0L)
        if (position <= 0 || updatedAt <= 0) return null
        return PlaybackCheckpoint(
            audiobookId = id,
            positionMs = position,
            durationMs = prefs.getLong(KEY_DURATION_MS, 0L),
            updatedAtMs = updatedAt,
            status = prefs.getString(KEY_STATUS, null),
            src = prefs.getString(KEY_SRC, null),
            title = prefs.getString(KEY_TITLE, null),
            artist = prefs.getString(KEY_ARTIST, null),
            coverPath = prefs.getString(KEY_COVER_PATH, null),
            chaptersJson = prefs.getString(KEY_CHAPTERS_JSON, null),
            speed = prefs.getFloat(KEY_SPEED, 1f),
            keepScreenOn = prefs.getBoolean(KEY_KEEP_SCREEN_ON, false),
        )
    }

    fun clearCheckpoint(context: Context) {
        prefs(context).edit().clear().apply()
        synchronized(lock) { lastCheckpointAt = 0L }
    }

    fun setSleepTimer(context: Context, minutes: Int) {
        synchronized(lock) {
            cancelSleepTimerLocked()
            if (minutes <= 0) {
                emit("tick")
                return
            }
            sleepDeadlineElapsed = SystemClock.elapsedRealtime() + minutes * 60_000L
            val runnable = Runnable {
                synchronized(lock) {
                    sleepDeadlineElapsed = 0L
                    sleepTimer = null
                    if (player?.playWhenReady == true) {
                        player?.pause()
                        saveCheckpoint(force = true)
                    }
                }
                emit("sleep")
            }
            sleepTimer = runnable
            sleepHandler.postDelayed(runnable, minutes * 60_000L)
        }
        emit("tick")
    }

    fun setKeepScreenOn(context: Context, enabled: Boolean) {
        synchronized(lock) {
            keepScreenOn = enabled
            saveCheckpoint(force = true)
        }
    }

    fun shouldKeepScreenOn(): Boolean = synchronized(lock) { keepScreenOn }

    fun isPlaying(): Boolean = synchronized(lock) {
        player?.let { it.playWhenReady && it.playbackState != Player.STATE_ENDED } == true
    }

    fun mediaSession(): MediaSession? = synchronized(lock) { mediaSession }

    fun onServiceDestroyed() {
        synchronized(lock) {
            saveCheckpoint(force = true)
            tickHandler.removeCallbacks(ticker)
            tickScheduled = false
            cancelSleepTimerLocked()
            runCatching { player?.removeListener(playerListener) }
            runCatching { player?.release() }
            runCatching { mediaSession?.release() }
            player = null
            sessionPlayer = null
            mediaSession = null
            currentId = null
            title = null
            artist = null
            src = null
            coverPath = null
            chaptersJson = null
            chapters = emptyList()
            lastError = null
            ended = false
            lastCheckpointAt = 0L
        }
    }

    private fun startService(context: Context) {
        runCatching {
            context.startService(Intent(context, AudiobookPlaybackService::class.java))
        }.onFailure { error ->
            Log.w(TAG, "no se pudo iniciar el servicio de reproducción", error)
        }
    }

    private fun syncTicker() {
        synchronized(lock) { syncTickerLocked() }
    }

    private fun syncTickerLocked() {
        val exo = player
        val active = exo != null &&
            exo.playWhenReady &&
            exo.playbackState != Player.STATE_ENDED &&
            exo.playbackState != Player.STATE_IDLE
        if (active && !tickScheduled) {
            tickScheduled = true
            tickHandler.removeCallbacks(ticker)
            tickHandler.post(ticker)
        } else if (!active && tickScheduled) {
            tickScheduled = false
            tickHandler.removeCallbacks(ticker)
        }
    }

    private fun seekChapter(direction: Int) {
        synchronized(lock) {
            val exo = player ?: return
            if (chapters.isEmpty()) {
                if (direction < 0) exo.seekBack() else exo.seekForward()
                return
            }
            val position = exo.currentPosition
            val current = chapterIndexAt(position)
            val target = if (direction < 0) {
                val chapterStart = chapters[current].startMs
                if (position - chapterStart > 3_000L) chapterStart
                else chapters[(current - 1).coerceAtLeast(0)].startMs
            } else {
                chapters[(current + 1).coerceAtMost(chapters.lastIndex)].startMs
            }
            exo.seekTo(target)
        }
    }

    private fun stateLocked(event: String): PlaybackState {
        val exo = player
        val rawPosition = exo?.currentPosition?.coerceAtLeast(0) ?: 0L
        val rawDuration = exo?.duration?.takeIf { it > 0 } ?: 0L
        val playbackEnded = ended || exo?.playbackState == Player.STATE_ENDED
        val position = if (playbackEnded && rawDuration > 0) rawDuration else rawPosition
        val index = chapterIndexAt(position)
        val remaining = sleepDeadlineElapsed.let { deadline ->
            if (deadline > 0) (deadline - SystemClock.elapsedRealtime()).coerceAtLeast(0) else 0
        }
        val playing = exo != null && exo.playWhenReady && !playbackEnded
        return PlaybackState(
            event = event,
            audiobookId = currentId,
            positionMs = position,
            durationMs = rawDuration,
            playing = playing,
            ended = playbackEnded,
            speed = speed,
            chapterIndex = index,
            chapterTitle = chapters.getOrNull(index)?.title,
            sleepRemainingMs = remaining,
            error = lastError,
        )
    }

    private fun chapterIndexAt(positionMs: Long): Int {
        if (chapters.isEmpty()) return 0
        val position = positionMs.coerceAtLeast(0)
        return chapters.indexOfLast { position >= it.startMs }.coerceAtLeast(0)
    }

    private fun mimeTypeFor(path: String): String? {
        return when (path.substringAfterLast('.', "").lowercase()) {
            "m4b", "m4a", "mp4" -> MimeTypes.AUDIO_MP4
            "mp3" -> MimeTypes.AUDIO_MPEG
            "aac" -> MimeTypes.AUDIO_AAC
            "ogg", "oga" -> MimeTypes.AUDIO_OGG
            "opus" -> MimeTypes.AUDIO_OPUS
            "flac" -> MimeTypes.AUDIO_FLAC
            else -> null
        }
    }

    private fun parseChapters(raw: String?): List<PlaybackChapter> {
        if (raw.isNullOrBlank()) return emptyList()
        return runCatching {
            val array = JSONArray(raw)
            (0 until array.length()).mapNotNull { index ->
                val item = array.optJSONObject(index) ?: return@mapNotNull null
                val title = item.optString("title").trim()
                PlaybackChapter(
                    startMs = item.optLong("startMs", 0L).coerceAtLeast(0L),
                    title = title.ifEmpty { "Capítulo ${index + 1}" },
                )
            }.sortedBy { it.startMs }
        }.getOrDefault(emptyList())
    }

    private fun emit(kind: String) {
        val channel: Channel
        val payload: JSObject
        synchronized(lock) {
            channel = eventChannel ?: return
            payload = stateLocked(kind).toJsObject()
        }
        runCatching { channel.send(payload) }
            .onFailure { Log.w(TAG, "no se pudo enviar el estado de reproducción", it) }
    }

    private fun saveCheckpoint(force: Boolean) {
        val ctx = context ?: return
        val id = currentId ?: return
        val exo = player ?: return
        val now = System.currentTimeMillis()
        if (!force && now - lastCheckpointAt < CHECKPOINT_INTERVAL_MS) return
        lastCheckpointAt = now
        val position = exo.currentPosition.coerceAtLeast(0)
        val duration = exo.duration.takeIf { it > 0 } ?: 0L
        prefs(ctx).edit()
            .putString(KEY_AUDIOBOOK_ID, id)
            .putLong(KEY_POSITION_MS, position)
            .putLong(KEY_DURATION_MS, duration)
            .putLong(KEY_UPDATED_AT_MS, now)
            .putString(KEY_STATUS, if (ended) "ended" else if (exo.playWhenReady) "playing" else "paused")
            .putString(KEY_SRC, src)
            .putString(KEY_TITLE, title)
            .putString(KEY_ARTIST, artist)
            .putString(KEY_COVER_PATH, coverPath)
            .putString(KEY_CHAPTERS_JSON, chaptersJson)
            .putFloat(KEY_SPEED, speed)
            .putBoolean(KEY_KEEP_SCREEN_ON, keepScreenOn)
            .apply()
    }

    private fun cancelSleepTimerLocked() {
        sleepTimer?.let { sleepHandler.removeCallbacks(it) }
        sleepTimer = null
        sleepDeadlineElapsed = 0L
    }

    private fun prefs(context: Context): SharedPreferences =
        context.applicationContext.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
}
