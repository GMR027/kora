package dev.kora.reader.audio

import android.content.Intent
import android.util.Log
import androidx.media3.session.DefaultMediaNotificationProvider
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import dev.kora.reader.R

/**
 * Servicio en primer plano que mantiene el audio y la notificación vivos
 * cuando la app queda en segundo plano o se apaga la pantalla.
 */
class AudiobookPlaybackService : MediaSessionService() {
    override fun onCreate() {
        super.onCreate()

        val provider = DefaultMediaNotificationProvider.Builder(this)
            .setNotificationId(NOTIFICATION_ID)
            .setChannelName(R.string.audiobook_notification_channel)
            .build()
        provider.setSmallIcon(R.drawable.ic_stat_headphones)
        setMediaNotificationProvider(provider)
        setShowNotificationForIdlePlayer(SHOW_NOTIFICATION_FOR_IDLE_PLAYER_NEVER)

        runCatching { PlaybackRuntime.ensure(applicationContext) }
            .onFailure { error ->
                Log.e("KoraAudio", "no se pudo inicializar el reproductor", error)
                stopSelf()
            }
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? =
        PlaybackRuntime.mediaSession()

    override fun onTaskRemoved(rootIntent: Intent?) {
        // Si se está reproduciendo, la barra de tareas puede desaparecer y el
        // audio debe seguir como en cualquier reproductor de música.
        if (!PlaybackRuntime.isPlaying()) {
            stopSelf()
        }
    }

    override fun onDestroy() {
        PlaybackRuntime.onServiceDestroyed()
        super.onDestroy()
    }

    private companion object {
        const val NOTIFICATION_ID = 7412
    }
}
