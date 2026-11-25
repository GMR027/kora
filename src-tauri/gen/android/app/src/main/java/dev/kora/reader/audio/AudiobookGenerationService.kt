package dev.kora.reader.audio

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.Build
import android.os.IBinder
import android.os.PowerManager
import androidx.core.app.NotificationCompat
import androidx.core.content.ContextCompat
import dev.kora.reader.MainActivity
import dev.kora.reader.R

/**
 * Mantiene vivo el proceso mientras Kora sintetiza y empaqueta un audiolibro
 * con MediaCodec. Sin el servicio en primer plano Android puede suspender la
 * app al apagar la pantalla y perder la generación a medias.
 */
class AudiobookGenerationService : Service() {
    companion object {
        private const val CHANNEL_ID = "kora-generation"
        private const val NOTIFICATION_ID = 6402
        private const val ACTION_START = "dev.kora.reader.GENERATION_START"
        private const val ACTION_UPDATE = "dev.kora.reader.GENERATION_UPDATE"
        private const val ACTION_STOP = "dev.kora.reader.GENERATION_STOP"
        private const val EXTRA_TITLE = "title"
        private const val EXTRA_MESSAGE = "message"
        private const val EXTRA_PERCENT = "percent"

        @Volatile
        private var running = false

        fun start(context: Context, title: String) {
            running = true
            val intent = Intent(context, AudiobookGenerationService::class.java).apply {
                action = ACTION_START
                putExtra(EXTRA_TITLE, title)
                putExtra(EXTRA_MESSAGE, "Preparando la síntesis…")
                putExtra(EXTRA_PERCENT, 0)
            }
            ContextCompat.startForegroundService(context, intent)
        }

        fun update(context: Context, percent: Float, message: String) {
            if (!running) return
            val intent = Intent(context, AudiobookGenerationService::class.java).apply {
                action = ACTION_UPDATE
                putExtra(EXTRA_MESSAGE, message)
                putExtra(EXTRA_PERCENT, percent.toInt().coerceIn(0, 100))
            }
            runCatching { context.startService(intent) }
        }

        fun stop(context: Context) {
            if (!running) return
            running = false
            val intent = Intent(context, AudiobookGenerationService::class.java).apply {
                action = ACTION_STOP
            }
            runCatching { context.startService(intent) }
        }
    }

    private var wakeLock: PowerManager.WakeLock? = null
    private var activeTitle = "Generando audiolibro"

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_UPDATE -> {
                val message = intent.getStringExtra(EXTRA_MESSAGE) ?: "Generando…"
                val percent = intent.getIntExtra(EXTRA_PERCENT, 0)
                notify(percent, message, ongoing = true)
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                    startForegroundCompat(percent, message)
                }
                return START_NOT_STICKY
            }

            ACTION_STOP -> {
                releaseWakeLock()
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                    stopForeground(STOP_FOREGROUND_REMOVE)
                } else {
                    @Suppress("DEPRECATION")
                    stopForeground(true)
                }
                stopSelf()
                return START_NOT_STICKY
            }
        }

        val title = intent?.getStringExtra(EXTRA_TITLE) ?: "Audiolibro"
        activeTitle = title
        val message = intent?.getStringExtra(EXTRA_MESSAGE) ?: "Preparando la síntesis…"
        acquireWakeLock()
        startForegroundCompat(0, message, title)
        return START_NOT_STICKY
    }

    private fun startForegroundCompat(
        percent: Int,
        message: String,
        title: String? = null,
    ) {
        val notification = buildNotification(percent, message, title ?: activeTitle)
        when {
            Build.VERSION.SDK_INT >= 35 -> startForeground(
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_MEDIA_PROCESSING,
            )

            Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q -> startForeground(
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_DATA_SYNC,
            )

            else -> startForeground(NOTIFICATION_ID, notification)
        }
    }

    private fun notify(percent: Int, message: String, ongoing: Boolean) {
        val manager = getSystemService(NotificationManager::class.java)
        manager.notify(
            NOTIFICATION_ID,
            buildNotification(percent, message, activeTitle, ongoing),
        )
    }

    private fun buildNotification(
        percent: Int,
        message: String,
        title: String,
        ongoing: Boolean = true,
    ) = NotificationCompat.Builder(this, CHANNEL_ID)
        .setSmallIcon(R.drawable.ic_stat_headphones)
        .setContentTitle(title)
        .setContentText(message)
        .setOnlyAlertOnce(true)
        .setOngoing(ongoing)
        .setSilent(true)
        .setProgress(100, percent.coerceIn(0, 100), false)
        .setContentIntent(
            PendingIntent.getActivity(
                this,
                0,
                Intent(this, MainActivity::class.java).apply {
                    flags = Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP
                },
                PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
            ),
        )
        .build()

    private fun acquireWakeLock() {
        if (wakeLock?.isHeld == true) return
        val power = getSystemService(Context.POWER_SERVICE) as PowerManager
        wakeLock = power.newWakeLock(
            PowerManager.PARTIAL_WAKE_LOCK,
            "Kora:generation",
        ).apply {
            setReferenceCounted(false)
            acquire()
        }
    }

    private fun releaseWakeLock() {
        wakeLock?.let { lock ->
            if (lock.isHeld) lock.release()
        }
        wakeLock = null
    }

    override fun onCreate() {
        super.onCreate()
        val manager = getSystemService(NotificationManager::class.java)
        if (manager.getNotificationChannel(CHANNEL_ID) == null) {
            manager.createNotificationChannel(
                NotificationChannel(
                    CHANNEL_ID,
                    "Generación de audiolibros",
                    NotificationManager.IMPORTANCE_LOW,
                ).apply {
                    description = "Progreso de la síntesis y el empaquetado en segundo plano."
                    setShowBadge(false)
                },
            )
        }
    }

    override fun onDestroy() {
        releaseWakeLock()
        running = false
        super.onDestroy()
    }
}
