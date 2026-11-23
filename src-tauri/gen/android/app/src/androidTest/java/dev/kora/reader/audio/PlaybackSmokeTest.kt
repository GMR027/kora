package dev.kora.reader.audio

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.PI
import kotlin.math.abs
import kotlin.math.sin

@RunWith(AndroidJUnit4::class)
class PlaybackSmokeTest {
    @Test
    fun playsPausesSeeksAndStops() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val context = instrumentation.targetContext
        val audio = File(context.cacheDir, "kora-playback-smoke.wav")
        writeWav(audio)

        val args = PlayArgs().apply {
            audiobookId = "playback-smoke-test"
            src = audio.absolutePath
            title = "Prueba de reproducción"
            artist = "Kora"
            startMs = 0
            speed = 1f
            chaptersJson =
                """[{"startMs":0,"title":"Uno"},{"startMs":700,"title":"Dos"}]"""
            keepScreenOn = false
        }

        var state: PlaybackState? = null
        instrumentation.runOnMainSync { state = PlaybackRuntime.play(context, args) }
        assertTrue(state?.playing == true)

        Thread.sleep(1_200)
        instrumentation.runOnMainSync { state = PlaybackRuntime.getState(context) }
        assertTrue("la posición debe avanzar", (state?.positionMs ?: 0) > 0)

        instrumentation.runOnMainSync { state = PlaybackRuntime.pause(context) }
        assertFalse(state?.playing == true)

        instrumentation.runOnMainSync { state = PlaybackRuntime.seek(context, 900) }
        assertTrue(abs((state?.positionMs ?: 0) - 900) < 80)

        instrumentation.runOnMainSync { state = PlaybackRuntime.resume(context) }
        Thread.sleep(300)
        instrumentation.runOnMainSync { state = PlaybackRuntime.getState(context) }
        assertTrue((state?.positionMs ?: 0) > 900)

        instrumentation.runOnMainSync { PlaybackRuntime.stop(context) }
        Thread.sleep(200)
        instrumentation.runOnMainSync { state = PlaybackRuntime.getState(context) }
        assertNull(state?.audiobookId)
    }

    private fun writeWav(file: File) {
        val sampleRate = 8_000
        val durationSeconds = 2
        val samples = sampleRate * durationSeconds
        val dataSize = samples * 2
        val buffer = ByteBuffer.allocate(44 + dataSize).order(ByteOrder.LITTLE_ENDIAN)
        buffer.put("RIFF".toByteArray())
        buffer.putInt(36 + dataSize)
        buffer.put("WAVE".toByteArray())
        buffer.put("fmt ".toByteArray())
        buffer.putInt(16)
        buffer.putShort(1)
        buffer.putShort(1)
        buffer.putInt(sampleRate)
        buffer.putInt(sampleRate * 2)
        buffer.putShort(2)
        buffer.putShort(16)
        buffer.put("data".toByteArray())
        buffer.putInt(dataSize)
        for (index in 0 until samples) {
            val sample = (sin(2 * PI * 440 * index / sampleRate) * Short.MAX_VALUE * 0.2).toInt()
            buffer.putShort(sample.toShort())
        }
        file.writeBytes(buffer.array())
    }
}
