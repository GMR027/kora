package dev.kora.reader.tts

import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import android.speech.tts.TextToSpeech
import android.speech.tts.UtteranceProgressListener
import android.util.Log
import androidx.media3.session.MediaSession
import app.tauri.plugin.Channel
import app.tauri.plugin.JSObject
import java.io.File
import java.util.Locale
import java.util.concurrent.CompletableFuture
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import java.util.concurrent.TimeoutException

/**
 * Estado de voz compartido por el plugin Tauri y el servicio de reproducción.
 *
 * `TextToSpeech` vive aquí, con el contexto de aplicación, para que la
 * recreación de la Activity no corte la lectura. El `MediaSession` se crea en
 * el hilo principal y lo publica [TtsPlaybackService].
 */
object TtsPlaybackRuntime {
    private const val TAG = "KoraTts"
    private const val WAKE_TAG = "Kora:TtsPlayback"
    private const val STOP_SERVICE_GRACE_MS = 500L

    private val mainHandler = Handler(Looper.getMainLooper())
    private val lock = Any()

    private var appContext: Context? = null
    private var player: TtsPlayer? = null
    private var session: MediaSession? = null
    @Volatile private var model = TtsPlayer.Model()

    private var engine: TextToSpeech? = null
    @Volatile private var engineReady = false
    private var engineError: String? = null
    private var voiceCache: List<TtsVoiceInfo> = emptyList()

    private var currentUtteranceId: String? = null
    @Volatile private var utteranceState = "idle"
    @Volatile private var utteranceError: String? = null

    /** Síntesis a archivo en curso, para las vistas previas del estudio de voz. */
    private val synthWaiters = ConcurrentHashMap<String, CompletableFuture<String?>>()

    private var controlChannel: Channel? = null
    private var focus: TtsAudioFocus? = null
    private var wakeLock: PowerManager.WakeLock? = null
    private var stopServiceRunnable: Runnable? = null

    private var defaultArtwork: ByteArray? = null
    private var defaultArtworkKey: String? = null

    fun ensure(context: Context) {
        val app = context.applicationContext
        synchronized(lock) {
            if (appContext == null) {
                appContext = app
                focus = TtsAudioFocus(app) { reason ->
                    onAudioFocusLost(reason)
                }
                initializeEngine(app)
            } else if (engine == null || (!engineReady && engineError != null)) {
                initializeEngine(app)
            }
        }
        if (Looper.myLooper() == Looper.getMainLooper()) {
            ensurePlayer()
        } else {
            mainHandler.post { ensurePlayer() }
        }
    }

    fun setControlChannel(channel: Channel?) {
        controlChannel = channel
    }

    fun voices(): List<TtsVoiceInfo> {
        refreshVoices()
        return TtsVoiceCatalog.listable(voiceCache)
    }

    fun speak(
        text: String,
        language: String?,
        voiceId: String?,
        rate: Float,
        pitch: Float,
        volume: Float,
    ): String? {
        val tts = engine
        if (tts == null || !engineReady) {
            return engineError ?: "El motor TTS de Android todavía no está listo"
        }
        if (text.isBlank()) return "No hay texto para leer"

        refreshVoices()
        if (voiceCache.none { it.isUsable() }) {
            return "No hay ninguna voz instalada en el motor TTS de Android"
        }

        val selectionError = selectVoice(tts, voiceId, language)
        if (selectionError != null) return selectionError

        tts.setSpeechRate(rate.coerceIn(0.5f, 3.0f))
        tts.setPitch(pitch.coerceIn(0.5f, 2.0f))
        focus?.request()

        val utteranceId = "tts_${System.nanoTime()}"
        currentUtteranceId = utteranceId
        utteranceState = "starting"
        utteranceError = null

        val params = if (volume < 1f) {
            Bundle().apply { putFloat(TextToSpeech.Engine.KEY_PARAM_VOLUME, volume.coerceIn(0f, 1f)) }
        } else {
            null
        }

        val result = tts.speak(text, TextToSpeech.QUEUE_FLUSH, params, utteranceId)
        if (result != TextToSpeech.SUCCESS) {
            utteranceState = "error"
            utteranceError = "El motor TTS rechazó la locución"
            focus?.release()
            return utteranceError
        }
        return null
    }

    /**
     * Sintetiza a un WAV para la vista previa del estudio de voz.
     *
     * `TextToSpeech` no expone PCM en memoria, pero sí puede volcar la
     * locución a un archivo. Devolvemos el error como texto o `null` si el
     * archivo quedó listo.
     */
    fun synthesizeToFile(
        text: String,
        language: String?,
        voiceId: String?,
        rate: Float,
        pitch: Float,
        output: File,
    ): String? {
        val tts = engine
        if (tts == null || !engineReady) {
            return engineError ?: "El motor TTS de Android todavía no está listo"
        }
        if (text.isBlank()) return "No hay texto para leer"

        refreshVoices()
        if (voiceCache.none { it.isUsable() }) {
            return "No hay ninguna voz instalada en el motor TTS de Android"
        }

        val selectionError = selectVoice(tts, voiceId, language)
        if (selectionError != null) return selectionError

        tts.setSpeechRate(rate.coerceIn(0.5f, 3.0f))
        tts.setPitch(pitch.coerceIn(0.5f, 2.0f))

        val utteranceId = "synth_${System.nanoTime()}"
        val completion = CompletableFuture<String?>()
        synthWaiters[utteranceId] = completion

        val result = tts.synthesizeToFile(text, null, output, utteranceId)
        if (result != TextToSpeech.SUCCESS) {
            synthWaiters.remove(utteranceId)
            output.delete()
            return "El motor TTS rechazó la síntesis de voz"
        }

        return try {
            val error = completion.get(45, TimeUnit.SECONDS)
            if (error != null) output.delete()
            error
        } catch (timeout: TimeoutException) {
            output.delete()
            "La síntesis de voz tardó demasiado"
        } catch (error: Exception) {
            output.delete()
            error.message ?: "Error de síntesis de voz"
        } finally {
            synthWaiters.remove(utteranceId)
        }
    }

    fun utteranceStatus(): Pair<String, String?> = utteranceState to utteranceError

    private fun completeSynthesis(utteranceId: String?, error: String?) {
        if (utteranceId == null) return
        synthWaiters.remove(utteranceId)?.complete(error)
    }

    fun stopSpeech() {
        val tts = engine ?: return
        tts.stop()
        focus?.release()
        if (utteranceState == "starting" || utteranceState == "speaking") {
            utteranceState = "interrupted"
        }
    }

    fun startSession(
        title: String,
        artist: String,
        album: String?,
        coverPath: String?,
    ) {
        val ctx = appContext ?: return
        val coverUri = coverPath
            ?.takeIf { it.isNotBlank() }
            ?.let(::File)
            ?.takeIf { it.isFile }
            ?.let { Uri.fromFile(it) }
        val artwork = if (coverUri == null) defaultArtwork(title, album) else null
        model = TtsPlayer.Model(
            title = title.ifBlank { "Kora" },
            artist = artist.ifBlank { "Lectura en voz alta" },
            album = album,
            coverUri = coverUri,
            artworkData = artwork,
            active = true,
            playing = true,
            speed = 1f,
        )
        mainHandler.post {
            cancelStopService()
            ensurePlayer()
            player?.update(model)
            acquireWakeLock()
            startService(ctx)
        }
    }

    fun setSessionState(state: String, error: String?) {
        val ctx = appContext
        mainHandler.post {
            when (state) {
                "playing" -> {
                    model = model.copy(active = true, playing = true)
                    cancelStopService()
                    ensurePlayer()
                    player?.update(model)
                    acquireWakeLock()
                    ctx?.let(::startService)
                }
                "paused" -> {
                    model = model.copy(playing = false)
                    player?.update(model)
                    releaseWakeLock()
                }
                "stopped", "finished", "error" -> {
                    if (state == "error" && !error.isNullOrBlank()) {
                        Log.w(TAG, "la sesión TTS terminó con error: $error")
                    }
                    model = model.copy(active = false, playing = false)
                    player?.update(model)
                    releaseWakeLock()
                    // Pequeña ventana para que el autoavance de capítulo
                    // reutilice el servicio en vez de destruirlo y recrearlo.
                    ctx?.let(::scheduleStopService)
                }
                else -> Log.w(TAG, "estado de sesión TTS desconocido: $state")
            }
        }
    }

    fun stopSession() {
        setSessionState("stopped", null)
    }

    fun isActive(): Boolean = model.active

    fun mediaSession(): MediaSession? = session

    fun onServiceDestroyed(destroyedSession: MediaSession?) {
        mainHandler.post {
            // Un cierre tardío del servicio anterior no debe destruir la sesión
            // de un capítulo nuevo que ya arrancó. Si hay una sesión activa,
            // el servicio se recreará con el mismo MediaSession.
            if (model.active) return@post
            if (destroyedSession != null && destroyedSession !== session) return@post
            cancelStopService()
            if (utteranceState == "starting" || utteranceState == "speaking") {
                stopSpeech()
                emitControl("stop")
            }
            releaseWakeLock()
            runCatching { session?.release() }
            session = null
            runCatching { player?.release() }
            player = null
            model = model.copy(active = false, playing = false)
        }
    }

    private fun initializeEngine(context: Context) {
        engineError = null
        engineReady = false
        val created = TextToSpeech(context.applicationContext) { status ->
            if (status == TextToSpeech.SUCCESS) {
                engineReady = true
                engineError = null
                engine?.setOnUtteranceProgressListener(progressListener)
                refreshVoices()
            } else {
                engineReady = false
                engineError = "El motor TTS de Android no pudo inicializarse"
                Log.e(TAG, "TextToSpeech init status=$status")
            }
        }
        engine = created
        // Si el motor respondió de forma síncrona antes de la asignación
        // anterior, el listener todavía no estaba enganchado.
        if (engineReady) {
            created.setOnUtteranceProgressListener(progressListener)
            refreshVoices()
        }
    }

    private fun refreshVoices() {
        val tts = engine ?: return
        if (!engineReady) return
        val native = tts.voices ?: return
        if (native.isEmpty()) return
        voiceCache = native.map { TtsVoiceCatalog.from(it, tts) }
    }

    private fun selectVoice(engine: TextToSpeech, voiceId: String?, language: String?): String? {
        val usable = voiceCache.filter { it.isUsable() }
        if (voiceId != null && voiceId.isNotBlank()) {
            val selected = usable.firstOrNull { it.id == voiceId }
            if (selected != null) {
                setVoiceById(engine, selected.id)
                return null
            }
            val fallback = TtsVoiceCatalog.fallbackFor(usable, voiceId)
            if (fallback != null) {
                setVoiceById(engine, fallback.id)
                return null
            }
            return "La voz seleccionada no está instalada"
        }

        if (!language.isNullOrBlank()) {
            val result = engine.setLanguage(Locale.forLanguageTag(language))
            if (result != TextToSpeech.LANG_MISSING_DATA &&
                result != TextToSpeech.LANG_NOT_SUPPORTED
            ) {
                return null
            }
        }

        if (engine.voice == null) {
            TtsVoiceCatalog.defaultVoice(usable)?.let { setVoiceById(engine, it.id) }
        }
        return null
    }

    private fun setVoiceById(engine: TextToSpeech, id: String) {
        engine.voices.orEmpty().firstOrNull { it.name == id }?.let { engine.voice = it }
    }

    private fun ensurePlayer() {
        val ctx = appContext ?: return
        if (player != null) return
        val ttsPlayer = TtsPlayer(Looper.getMainLooper(), ::emitControl)
        val launchIntent = ctx.packageManager.getLaunchIntentForPackage(ctx.packageName)
        val pendingIntent = launchIntent?.let {
            PendingIntent.getActivity(
                ctx,
                0,
                it,
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            )
        }
        session = MediaSession.Builder(ctx, ttsPlayer)
            .setId("kora-tts")
            .apply { if (pendingIntent != null) setSessionActivity(pendingIntent) }
            .build()
        player = ttsPlayer
        ttsPlayer.update(model)
    }

    private fun startService(context: Context) {
        runCatching {
            context.startService(Intent(context, TtsPlaybackService::class.java))
        }.onFailure { error ->
            Log.w(TAG, "no se pudo iniciar el servicio de reproducción TTS", error)
        }
    }

    private fun stopService(context: Context) {
        runCatching {
            context.stopService(Intent(context, TtsPlaybackService::class.java))
        }
    }

    private fun scheduleStopService(context: Context) {
        cancelStopService()
        val runnable = Runnable {
            stopServiceRunnable = null
            stopService(context)
        }
        stopServiceRunnable = runnable
        mainHandler.postDelayed(runnable, STOP_SERVICE_GRACE_MS)
    }

    private fun cancelStopService() {
        stopServiceRunnable?.let(mainHandler::removeCallbacks)
        stopServiceRunnable = null
    }

    private fun onAudioFocusLost(reason: String) {
        Log.i(TAG, "audio focus perdido: $reason")
        stopSpeech()
        model = model.copy(playing = false)
        mainHandler.post {
            player?.update(model)
            releaseWakeLock()
            emitControl("pause")
        }
    }

    private fun emitControl(action: String) {
        val channel = controlChannel ?: return
        runCatching { channel.send(JSObject().put("action", action)) }
            .onFailure { Log.w(TAG, "no se pudo enviar la acción $action", it) }
    }

    private fun acquireWakeLock() {
        val ctx = appContext ?: return
        val lock = wakeLock ?: run {
            val power = ctx.getSystemService(Context.POWER_SERVICE) as PowerManager
            power.newWakeLock(PowerManager.PARTIAL_WAKE_LOCK, WAKE_TAG).apply {
                setReferenceCounted(false)
            }.also { wakeLock = it }
        }
        if (!lock.isHeld) lock.acquire()
    }

    private fun releaseWakeLock() {
        val lock = wakeLock ?: return
        if (lock.isHeld) lock.release()
    }

    private fun defaultArtwork(title: String, author: String?): ByteArray? {
        val key = "$title|${author.orEmpty()}"
        if (key == defaultArtworkKey) return defaultArtwork
        return runCatching {
            DefaultTtsArtwork.render(title, author)
        }.onSuccess {
            defaultArtwork = it
            defaultArtworkKey = key
        }.onFailure {
            Log.w(TAG, "no se pudo generar la portada de respaldo", it)
        }.getOrNull()
    }

    private val progressListener = object : UtteranceProgressListener() {
        override fun onStart(utteranceId: String?) {
            if (utteranceId == currentUtteranceId) {
                utteranceState = "speaking"
                utteranceError = null
            }
        }

        override fun onDone(utteranceId: String?) {
            completeSynthesis(utteranceId, null)
            if (utteranceId == currentUtteranceId) {
                utteranceState = "finished"
                utteranceError = null
                focus?.release()
            }
        }

        @Deprecated("Superseded by onError(String, Int)")
        override fun onError(utteranceId: String?) {
            completeSynthesis(utteranceId, "Error de síntesis de voz")
            if (utteranceId == currentUtteranceId) {
                utteranceState = "error"
                utteranceError = "Error de síntesis de voz"
                focus?.release()
            }
        }

        override fun onError(utteranceId: String?, errorCode: Int) {
            completeSynthesis(utteranceId, "Error de síntesis de voz ($errorCode)")
            if (utteranceId == currentUtteranceId) {
                utteranceState = "error"
                utteranceError = "Error de síntesis de voz ($errorCode)"
                focus?.release()
            }
        }

        override fun onStop(utteranceId: String?, interrupted: Boolean) {
            completeSynthesis(utteranceId, "La síntesis de voz se canceló")
            if (utteranceId == currentUtteranceId) {
                utteranceState = "interrupted"
                focus?.release()
            }
        }
    }
}
