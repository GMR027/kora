package dev.kora.reader.audio

import android.Manifest
import android.app.Activity
import android.content.pm.PackageManager
import android.os.Build
import android.view.WindowManager
import androidx.appcompat.app.AppCompatActivity
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

private const val NOTIFICATION_PERMISSION_REQUEST_CODE = 9513

@InvokeArg
class PlayArgs {
    var audiobookId: String = ""
    var src: String = ""
    var title: String = ""
    var artist: String? = null
    var coverPath: String? = null
    var startMs: Long = 0
    var speed: Float = 1f
    var chaptersJson: String? = null
    var keepScreenOn: Boolean = false
}

@InvokeArg
class SeekArgs {
    var positionMs: Long = 0
}

@InvokeArg
class SetSpeedArgs {
    var speed: Float = 1f
}

@InvokeArg
class SetSleepTimerArgs {
    var minutes: Int = 0
}

@InvokeArg
class SetKeepScreenOnArgs {
    var enabled: Boolean = false
}

@InvokeArg
class SetupEventRelayArgs {
    lateinit var channel: Channel
}

@InvokeArg
class DecodeAudioRangeArgs {
    lateinit var inputPath: String
    lateinit var outputPath: String
    var startMs: Long = 0
    var durationMs: Long = 0
}

@InvokeArg
class TranscriptionStartArgs {
    var title: String = ""
}

@InvokeArg
class TranscriptionUpdateArgs {
    var percent: Float = 0f
    var message: String = ""
}

@InvokeArg
class GenerationStartArgs {
    var title: String = ""
}

@InvokeArg
class GenerationUpdateArgs {
    var percent: Float = 0f
    var message: String = ""
}

@InvokeArg
class MuxAudiobookArgs {
    var chaptersJson: String = ""
    var outputPath: String = ""
    var title: String = ""
    var artist: String = ""
    var metadataJson: String? = null
}

@TauriPlugin
class AudiobookAudioPlugin(private val activity: Activity) : Plugin(activity) {
    @Command
    fun initialize(invoke: Invoke) {
        runCatching {
            requestNotificationPermission()
            PlaybackRuntime.getState(activity.applicationContext)
        }.onSuccess { state ->
            invoke.resolve(state.toJsObject())
        }.onFailure { error ->
            invoke.reject(error.message ?: "initialize failed")
        }
    }

    @Command
    fun decodeAudioRange(invoke: Invoke) {
        val args = invoke.parseArgs(DecodeAudioRangeArgs::class.java)
        Thread({
            runCatching {
                AudioRangeDecoder.decodeToWav(
                    inputPath = args.inputPath,
                    outputPath = args.outputPath,
                    startMs = args.startMs,
                    durationMs = args.durationMs,
                )
            }.onSuccess { result ->
                invoke.resolve(
                    JSObject()
                        .put("durationMs", result.durationMs)
                        .put("frames", result.frames)
                        .put("sampleRate", result.sampleRate),
                )
            }.onFailure { error ->
                invoke.reject(error.message ?: "decodeAudioRange failed")
            }
        }, "kora-audio-decode").start()
    }

    @Command
    fun startTranscription(invoke: Invoke) {
        val args = invoke.parseArgs(TranscriptionStartArgs::class.java)
        runCatching {
            TranscriptionService.start(activity.applicationContext, args.title)
        }.onSuccess {
            invoke.resolve(JSObject())
        }.onFailure { error ->
            invoke.reject(error.message ?: "startTranscription failed")
        }
    }

    @Command
    fun updateTranscription(invoke: Invoke) {
        val args = invoke.parseArgs(TranscriptionUpdateArgs::class.java)
        runCatching {
            TranscriptionService.update(
                activity.applicationContext,
                args.percent,
                args.message,
            )
        }.onSuccess {
            invoke.resolve(JSObject())
        }.onFailure { error ->
            invoke.reject(error.message ?: "updateTranscription failed")
        }
    }

    @Command
    fun stopTranscription(invoke: Invoke) {
        runCatching {
            TranscriptionService.stop(activity.applicationContext)
        }.onSuccess {
            invoke.resolve(JSObject())
        }.onFailure { error ->
            invoke.reject(error.message ?: "stopTranscription failed")
        }
    }

    @Command
    fun startGeneration(invoke: Invoke) {
        val args = invoke.parseArgs(GenerationStartArgs::class.java)
        runCatching {
            AudiobookGenerationService.start(activity.applicationContext, args.title)
        }.onSuccess {
            invoke.resolve(JSObject())
        }.onFailure { error ->
            invoke.reject(error.message ?: "startGeneration failed")
        }
    }

    @Command
    fun updateGeneration(invoke: Invoke) {
        val args = invoke.parseArgs(GenerationUpdateArgs::class.java)
        runCatching {
            AudiobookGenerationService.update(
                activity.applicationContext,
                args.percent,
                args.message,
            )
        }.onSuccess {
            invoke.resolve(JSObject())
        }.onFailure { error ->
            invoke.reject(error.message ?: "updateGeneration failed")
        }
    }

    @Command
    fun stopGeneration(invoke: Invoke) {
        runCatching {
            AudiobookGenerationService.stop(activity.applicationContext)
        }.onSuccess {
            invoke.resolve(JSObject())
        }.onFailure { error ->
            invoke.reject(error.message ?: "stopGeneration failed")
        }
    }

    @Command
    fun muxAudiobook(invoke: Invoke) {
        val args = invoke.parseArgs(MuxAudiobookArgs::class.java)
        Thread {
            runCatching {
                AudiobookMuxer.mux(
                    chaptersJson = args.chaptersJson,
                    outputPath = args.outputPath,
                    title = args.title,
                    artist = args.artist,
                    metadataJson = args.metadataJson,
                )
            }.onSuccess {
                invoke.resolve(JSObject())
            }.onFailure { error ->
                invoke.reject(error.message ?: "muxAudiobook failed")
            }
        }.start()
    }

    @Command
    fun setupEventRelay(invoke: Invoke) {
        val args = invoke.parseArgs(SetupEventRelayArgs::class.java)
        PlaybackRuntime.setEventChannel(args.channel)
        invoke.resolve()
    }

    @Command
    fun play(invoke: Invoke) {
        val args = invoke.parseArgs(PlayArgs::class.java)
        runCatching {
            requestNotificationPermission()
            val state = PlaybackRuntime.play(activity.applicationContext, args)
            applyKeepScreenOn(PlaybackRuntime.shouldKeepScreenOn() && state.playing)
            state
        }.onSuccess { state ->
            invoke.resolve(state.toJsObject())
        }.onFailure { error ->
            invoke.reject(error.message ?: "play failed")
        }
    }

    @Command
    fun resume(invoke: Invoke) {
        runCatching {
            val state = PlaybackRuntime.resume(activity.applicationContext)
            applyKeepScreenOn(PlaybackRuntime.shouldKeepScreenOn() && state.playing)
            state
        }.onSuccess { state ->
            invoke.resolve(state.toJsObject())
        }.onFailure { error ->
            invoke.reject(error.message ?: "resume failed")
        }
    }

    @Command
    fun pause(invoke: Invoke) {
        runCatching {
            val state = PlaybackRuntime.pause(activity.applicationContext)
            applyKeepScreenOn(false)
            state
        }.onSuccess { state ->
            invoke.resolve(state.toJsObject())
        }.onFailure { error ->
            invoke.reject(error.message ?: "pause failed")
        }
    }

    @Command
    fun seek(invoke: Invoke) {
        val args = invoke.parseArgs(SeekArgs::class.java)
        runCatching {
            PlaybackRuntime.seek(activity.applicationContext, args.positionMs)
        }.onSuccess { state ->
            invoke.resolve(state.toJsObject())
        }.onFailure { error ->
            invoke.reject(error.message ?: "seek failed")
        }
    }

    @Command
    fun setSpeed(invoke: Invoke) {
        val args = invoke.parseArgs(SetSpeedArgs::class.java)
        runCatching {
            PlaybackRuntime.setSpeed(activity.applicationContext, args.speed)
        }.onSuccess { state ->
            invoke.resolve(state.toJsObject())
        }.onFailure { error ->
            invoke.reject(error.message ?: "setSpeed failed")
        }
    }

    @Command
    fun stop(invoke: Invoke) {
        runCatching {
            val state = PlaybackRuntime.stop(activity.applicationContext)
            applyKeepScreenOn(false)
            state
        }.onSuccess { state ->
            invoke.resolve(state.toJsObject())
        }.onFailure { error ->
            invoke.reject(error.message ?: "stop failed")
        }
    }

    @Command
    fun getState(invoke: Invoke) {
        runCatching {
            PlaybackRuntime.getState(activity.applicationContext)
        }.onSuccess { state ->
            invoke.resolve(state.toJsObject())
        }.onFailure { error ->
            invoke.reject(error.message ?: "getState failed")
        }
    }

    @Command
    fun setSleepTimer(invoke: Invoke) {
        val args = invoke.parseArgs(SetSleepTimerArgs::class.java)
        runCatching {
            PlaybackRuntime.setSleepTimer(activity.applicationContext, args.minutes)
        }.onSuccess {
            invoke.resolve(PlaybackRuntime.getState(activity.applicationContext).toJsObject())
        }.onFailure { error ->
            invoke.reject(error.message ?: "setSleepTimer failed")
        }
    }

    @Command
    fun setKeepScreenOn(invoke: Invoke) {
        val args = invoke.parseArgs(SetKeepScreenOnArgs::class.java)
        runCatching {
            PlaybackRuntime.setKeepScreenOn(activity.applicationContext, args.enabled)
            applyKeepScreenOn(args.enabled && PlaybackRuntime.isPlaying())
        }.onSuccess {
            invoke.resolve()
        }.onFailure { error ->
            invoke.reject(error.message ?: "setKeepScreenOn failed")
        }
    }

    @Command
    fun getCheckpoint(invoke: Invoke) {
        runCatching {
            PlaybackRuntime.getCheckpoint(activity.applicationContext)
        }.onSuccess { checkpoint ->
            if (checkpoint == null) {
                invoke.resolve(null)
            } else {
                val payload = JSObject()
                payload.put("audiobookId", checkpoint.audiobookId)
                payload.put("positionMs", checkpoint.positionMs)
                payload.put("durationMs", checkpoint.durationMs)
                payload.put("updatedAtMs", checkpoint.updatedAtMs)
                payload.put("speed", checkpoint.speed.toDouble())
                payload.put("keepScreenOn", checkpoint.keepScreenOn)
                checkpoint.status?.let { payload.put("status", it) }
                checkpoint.src?.let { payload.put("src", it) }
                checkpoint.title?.let { payload.put("title", it) }
                checkpoint.artist?.let { payload.put("artist", it) }
                checkpoint.coverPath?.let { payload.put("coverPath", it) }
                checkpoint.chaptersJson?.let { payload.put("chaptersJson", it) }
                invoke.resolve(payload)
            }
        }.onFailure { error ->
            invoke.reject(error.message ?: "getCheckpoint failed")
        }
    }

    @Command
    fun clearCheckpoint(invoke: Invoke) {
        runCatching {
            PlaybackRuntime.clearCheckpoint(activity.applicationContext)
        }.onSuccess {
            invoke.resolve()
        }.onFailure { error ->
            invoke.reject(error.message ?: "clearCheckpoint failed")
        }
    }

    override fun onResume(activity: AppCompatActivity) {
        super.onResume(activity)
        applyKeepScreenOn(PlaybackRuntime.shouldKeepScreenOn() && PlaybackRuntime.isPlaying())
    }

    override fun onPause(activity: AppCompatActivity) {
        applyKeepScreenOn(false)
        super.onPause(activity)
    }

    override fun onDestroy(activity: AppCompatActivity) {
        applyKeepScreenOn(false)
        super.onDestroy(activity)
    }

    private fun applyKeepScreenOn(enabled: Boolean) {
        activity.runOnUiThread {
            if (enabled) {
                activity.window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            } else {
                activity.window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
            }
        }
    }

    private fun requestNotificationPermission() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        if (ContextCompat.checkSelfPermission(
                activity,
                Manifest.permission.POST_NOTIFICATIONS,
            ) == PackageManager.PERMISSION_GRANTED
        ) {
            return
        }
        ActivityCompat.requestPermissions(
            activity,
            arrayOf(Manifest.permission.POST_NOTIFICATIONS),
            NOTIFICATION_PERMISSION_REQUEST_CODE,
        )
    }
}
