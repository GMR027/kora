package dev.kora.reader.tts

import android.content.Intent
import android.util.Log
import androidx.media3.session.DefaultMediaNotificationProvider
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import dev.kora.reader.R

/**
 * Servicio en primer plano que mantiene viva la tarjeta de lectura en voz alta
 * con la pantalla bloqueada. El audio real lo sigue produciendo
 * `TextToSpeech`; aquí solo vive la sesión multimedia de Media3.
 */
class TtsPlaybackService : MediaSessionService() {
    private var ownedSession: MediaSession? = null

    override fun onCreate() {
        super.onCreate()
        runCatching {
            TtsPlaybackRuntime.ensure(applicationContext)
            ownedSession = TtsPlaybackRuntime.mediaSession()
            val provider = DefaultMediaNotificationProvider.Builder(this)
                .setNotificationId(NOTIFICATION_ID)
                .setChannelName(R.string.tts_notification_channel)
                .build()
            provider.setSmallIcon(R.drawable.ic_stat_headphones)
            setMediaNotificationProvider(provider)
            setShowNotificationForIdlePlayer(SHOW_NOTIFICATION_FOR_IDLE_PLAYER_NEVER)
        }.onFailure { error ->
            Log.e("KoraTts", "no se pudo inicializar el servicio TTS", error)
            stopSelf()
        }
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? =
        TtsPlaybackRuntime.mediaSession()

    override fun onTaskRemoved(rootIntent: Intent?) {
        // Si la lectura sigue activa, el usuario puede cerrar la tarea y
        // continuar escuchando desde la pantalla de bloqueo, como con un
        // audiolibro.
        if (!TtsPlaybackRuntime.isActive()) {
            stopSelf()
        }
    }

    override fun onDestroy() {
        TtsPlaybackRuntime.onServiceDestroyed(ownedSession)
        ownedSession = null
        super.onDestroy()
    }

    private companion object {
        const val NOTIFICATION_ID = 7413
    }
}
