package dev.kora.reader.tts

import android.app.Activity
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class TtsSpeakArgs {
    var text: String = ""
    var language: String? = null
    var voiceId: String? = null
    var rate: Float = 1f
    var pitch: Float = 1f
    var volume: Float = 1f
}

@InvokeArg
class TtsSynthesizeArgs {
    var text: String = ""
    var language: String? = null
    var voiceId: String? = null
    var rate: Float = 1f
    var pitch: Float = 1f
}

@InvokeArg
class TtsStartSessionArgs {
    var title: String = ""
    var artist: String = ""
    var album: String? = null
    var coverPath: String? = null
}

@InvokeArg
class TtsSessionStateArgs {
    var state: String = ""
    var error: String? = null
}

@InvokeArg
class TtsSetupControlRelayArgs {
    lateinit var channel: Channel
}

@InvokeArg
class TtsPlaySpeechFileArgs {
    var path: String = ""
    var volume: Float = 1f
}

@TauriPlugin
class KoraTtsPlugin(private val activity: Activity) : Plugin(activity) {
    private val runtime: TtsPlaybackRuntime
        get() {
            TtsPlaybackRuntime.ensure(activity.applicationContext)
            return TtsPlaybackRuntime
        }

    @Command
    fun getVoices(invoke: Invoke) {
        runCatching {
            runtime.voices()
        }.onSuccess { voices ->
            val array = JSArray()
            for (voice in voices) {
                array.put(
                    JSObject()
                        .put("id", voice.id)
                        .put("name", voice.displayName())
                        .put("language", voice.languageTag)
                        .put("locale", voice.languageTag)
                        .put("quality", voice.quality)
                        .put("latency", voice.latency)
                        .put("requiresNetwork", voice.requiresNetwork)
                )
            }
            invoke.resolve(JSObject().put("voices", array))
        }.onFailure { error ->
            invoke.reject(error.message ?: "no se pudo leer el catálogo de voces")
        }
    }

    @Command
    fun speak(invoke: Invoke) {
        val args = invoke.parseArgs(TtsSpeakArgs::class.java)
        runCatching {
            runtime.speak(
                text = args.text,
                language = args.language,
                voiceId = args.voiceId,
                rate = args.rate,
                pitch = args.pitch,
                volume = args.volume,
            )
        }.onSuccess { error ->
            if (error != null) {
                invoke.reject(error)
            } else {
                invoke.resolve(JSObject().put("success", true))
            }
        }.onFailure { error ->
            invoke.reject(error.message ?: "no se pudo iniciar la locución")
        }
    }

    @Command
    fun synthesize(invoke: Invoke) {
        val args = invoke.parseArgs(TtsSynthesizeArgs::class.java)
        // La síntesis a archivo puede tardar; se resuelve fuera del hilo
        // principal para no congelar la interfaz.
        Thread {
            runCatching {
                val output = java.io.File(
                    activity.cacheDir,
                    "tts-preview-${System.nanoTime()}.wav",
                )
                val error = runtime.synthesizeToFile(
                    text = args.text,
                    language = args.language,
                    voiceId = args.voiceId,
                    rate = args.rate,
                    pitch = args.pitch,
                    output = output,
                )
                if (error != null) {
                    output.delete()
                    invoke.reject(error)
                } else {
                    invoke.resolve(JSObject().put("path", output.absolutePath))
                }
            }.onFailure { error ->
                invoke.reject(error.message ?: "no se pudo sintetizar la voz")
            }
        }.start()
    }

    @Command
    fun utteranceStatus(invoke: Invoke) {
        val (state, error) = runtime.utteranceStatus()
        val payload = JSObject().put("state", state)
        if (!error.isNullOrBlank()) payload.put("error", error)
        invoke.resolve(payload)
    }

    @Command
    fun stop(invoke: Invoke) {
        runCatching {
            runtime.stopSpeech()
        }.onSuccess {
            invoke.resolve(JSObject().put("success", true))
        }.onFailure { error ->
            invoke.reject(error.message ?: "no se pudo detener la locución")
        }
    }

    @Command
    fun startSession(invoke: Invoke) {
        val args = invoke.parseArgs(TtsStartSessionArgs::class.java)
        runCatching {
            runtime.startSession(
                title = args.title,
                artist = args.artist,
                album = args.album,
                coverPath = args.coverPath,
            )
        }.onSuccess {
            invoke.resolve(JSObject().put("success", true))
        }.onFailure { error ->
            invoke.reject(error.message ?: "no se pudo iniciar la sesión de lectura")
        }
    }

    @Command
    fun setSessionState(invoke: Invoke) {
        val args = invoke.parseArgs(TtsSessionStateArgs::class.java)
        runCatching {
            runtime.setSessionState(args.state, args.error)
        }.onSuccess {
            invoke.resolve(JSObject().put("success", true))
        }.onFailure { error ->
            invoke.reject(error.message ?: "no se pudo actualizar la sesión de lectura")
        }
    }

    @Command
    fun stopSession(invoke: Invoke) {
        runCatching {
            runtime.stopSession()
        }.onSuccess {
            invoke.resolve(JSObject().put("success", true))
        }.onFailure { error ->
            invoke.reject(error.message ?: "no se pudo cerrar la sesión de lectura")
        }
    }

    @Command
    fun setupControlRelay(invoke: Invoke) {
        val args = invoke.parseArgs(TtsSetupControlRelayArgs::class.java)
        // No pasar por `runtime` aquí: eso inicializaría TextToSpeech y el
        // MediaSession al arrancar la app, antes de que el usuario pida leer.
        TtsPlaybackRuntime.setControlChannel(args.channel)
        invoke.resolve()
    }

    @Command
    fun playSpeechFile(invoke: Invoke) {
        val args = invoke.parseArgs(TtsPlaySpeechFileArgs::class.java)
        runCatching {
            SpeechFilePlayer.play(activity.applicationContext, args.path, args.volume)
        }.onSuccess {
            invoke.resolve()
        }.onFailure { error ->
            invoke.reject(error.message ?: "no se pudo reproducir el audio")
        }
    }

    @Command
    fun speechFileStatus(invoke: Invoke) {
        val (state, error) = SpeechFilePlayer.status()
        invoke.resolve(JSObject().put("state", state).put("error", error))
    }

    @Command
    fun pauseSpeechFile(invoke: Invoke) {
        SpeechFilePlayer.pause()
        invoke.resolve()
    }

    @Command
    fun resumeSpeechFile(invoke: Invoke) {
        SpeechFilePlayer.resume()
        invoke.resolve()
    }

    @Command
    fun stopSpeechFile(invoke: Invoke) {
        SpeechFilePlayer.stop()
        invoke.resolve()
    }
}
