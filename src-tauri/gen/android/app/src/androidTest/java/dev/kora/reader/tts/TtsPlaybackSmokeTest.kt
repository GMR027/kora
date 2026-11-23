package dev.kora.reader.tts

import android.os.Looper
import androidx.media3.common.Player
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class TtsPlaybackSmokeTest {
    @Test
    fun exposesMetadataAndTransportControls() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val controls = mutableListOf<String>()
        lateinit var player: TtsPlayer

        instrumentation.runOnMainSync {
            player = TtsPlayer(Looper.getMainLooper()) { controls.add(it) }
            player.update(
                TtsPlayer.Model(
                    title = "El arte de perder el rumbo",
                    artist = "Capítulo 7",
                    album = "Marta Salinas",
                    active = true,
                    playing = true,
                )
            )
        }

        instrumentation.runOnMainSync {
            assertEquals("El arte de perder el rumbo", player.mediaMetadata.title)
            assertEquals("Capítulo 7", player.mediaMetadata.artist)
            assertEquals("Marta Salinas", player.mediaMetadata.albumTitle)
            assertTrue(player.availableCommands.contains(Player.COMMAND_PLAY_PAUSE))
            assertTrue(player.availableCommands.contains(Player.COMMAND_STOP))
            player.pause()
        }

        assertEquals(listOf("pause"), controls)

        instrumentation.runOnMainSync { player.stop() }
        assertEquals(listOf("pause", "stop"), controls)
    }
}
