package dev.kora.reader.audio

import android.media.MediaCodec
import android.media.MediaCodecInfo
import android.media.MediaFormat
import android.media.MediaMuxer
import org.json.JSONArray
import java.io.Closeable
import java.io.File
import java.io.FileInputStream

/**
 * Empaqueta los WAV por capítulo que produce el motor TTS de Kora en un
 * MP4/AAC reproducible por Media3. Los capítulos siguen en el manifiesto JSON
 * de la aplicación; el contenedor no incluye átomos de capítulos.
 */
object AudiobookMuxer {
    private const val SAMPLE_RATE = 24_000
    private const val CHANNELS = 1
    private const val BIT_RATE = 96_000
    private const val TIMEOUT_US = 10_000L
    private const val INPUT_BUFFER_SIZE = 32 * 1024

    private data class Chapter(val path: String, val title: String)

    fun mux(
        chaptersJson: String,
        outputPath: String,
        title: String,
        artist: String,
        metadataJson: String?,
    ) {
        val chapters = parseChapters(chaptersJson)
        if (chapters.isEmpty()) error("No hay capítulos para codificar")
        val output = File(outputPath)
        output.parentFile?.mkdirs()
        if (output.exists() && !output.delete()) {
            error("No se pudo reemplazar el audiolibro existente")
        }

        val format = MediaFormat.createAudioFormat(
            MediaFormat.MIMETYPE_AUDIO_AAC,
            SAMPLE_RATE,
            CHANNELS,
        ).apply {
            setInteger(
                MediaFormat.KEY_AAC_PROFILE,
                MediaCodecInfo.CodecProfileLevel.AACObjectLC,
            )
            setInteger(MediaFormat.KEY_BIT_RATE, BIT_RATE)
            setInteger(MediaFormat.KEY_MAX_INPUT_SIZE, INPUT_BUFFER_SIZE)
        }
        val encoder = MediaCodec.createEncoderByType(MediaFormat.MIMETYPE_AUDIO_AAC)
        val muxer = MediaMuxer(outputPath, MediaMuxer.OutputFormat.MUXER_OUTPUT_MPEG_4)
        val bufferInfo = MediaCodec.BufferInfo()
        val pcmBuffer = ByteArray(INPUT_BUFFER_SIZE)
        var trackIndex = -1
        var muxerStarted = false
        var totalSamples = 0L

        fun drain(endOfStream: Boolean): Boolean {
            var attempts = 0
            while (true) {
                val outIndex = encoder.dequeueOutputBuffer(bufferInfo, TIMEOUT_US)
                when {
                    outIndex == MediaCodec.INFO_TRY_AGAIN_LATER -> {
                        if (!endOfStream) return false
                        if (++attempts > 2_000) {
                            error("El codificador AAC no terminó a tiempo")
                        }
                        Thread.sleep(5)
                    }

                    outIndex == MediaCodec.INFO_OUTPUT_FORMAT_CHANGED -> {
                        if (muxerStarted) error("El codificador AAC cambió de formato dos veces")
                        trackIndex = muxer.addTrack(encoder.outputFormat)
                        muxer.start()
                        muxerStarted = true
                    }

                    outIndex >= 0 -> {
                        val encoded = encoder.getOutputBuffer(outIndex) ?: continue
                        if (bufferInfo.flags and MediaCodec.BUFFER_FLAG_CODEC_CONFIG != 0) {
                            bufferInfo.size = 0
                        }
                        if (bufferInfo.size > 0 && muxerStarted) {
                            encoded.position(bufferInfo.offset)
                            encoded.limit(bufferInfo.offset + bufferInfo.size)
                            muxer.writeSampleData(trackIndex, encoded, bufferInfo)
                        }
                        encoder.releaseOutputBuffer(outIndex, false)
                        if (bufferInfo.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) {
                            return true
                        }
                    }
                }
            }
        }

        try {
            encoder.configure(format, null, null, MediaCodec.CONFIGURE_FLAG_ENCODE)
            encoder.start()
            for (chapter in chapters) {
                WavPcmReader(chapter.path).use { reader ->
                    while (true) {
                        val read = reader.read(pcmBuffer)
                        if (read <= 0) break
                        var offset = 0
                        while (offset < read) {
                            val inputIndex = encoder.dequeueInputBuffer(TIMEOUT_US)
                            if (inputIndex < 0) {
                                drain(false)
                                continue
                            }
                            val input = encoder.getInputBuffer(inputIndex)
                                ?: error("El codificador AAC no entregó un buffer de entrada")
                            input.clear()
                            var size = minOf(input.remaining(), read - offset)
                            size -= size % 2
                            if (size <= 0) {
                                // El buffer de entrada fue demasiado pequeño para
                                // una muestra completa; se devuelve vacío y se
                                // reintenta con otro buffer.
                                encoder.queueInputBuffer(inputIndex, 0, 0, 0, 0)
                                continue
                            }
                            input.put(pcmBuffer, offset, size)
                            val presentationTimeUs =
                                totalSamples * 1_000_000L / SAMPLE_RATE
                            encoder.queueInputBuffer(
                                inputIndex,
                                0,
                                size,
                                presentationTimeUs,
                                0,
                            )
                            offset += size
                            totalSamples += (size / 2).toLong()
                            drain(false)
                        }
                    }
                }
            }

            val inputIndex = encoder.dequeueInputBuffer(TIMEOUT_US)
            if (inputIndex < 0) {
                error("El codificador AAC no aceptó el final del flujo")
            }
            val input = encoder.getInputBuffer(inputIndex)
                ?: error("El codificador AAC no entregó el buffer final")
            input.clear()
            val presentationTimeUs = totalSamples * 1_000_000L / SAMPLE_RATE
            encoder.queueInputBuffer(
                inputIndex,
                0,
                0,
                presentationTimeUs,
                MediaCodec.BUFFER_FLAG_END_OF_STREAM,
            )
            drain(true)
        } finally {
            runCatching { encoder.stop() }
            encoder.release()
            if (muxerStarted) runCatching { muxer.stop() }
            muxer.release()
            if (!muxerStarted) {
                runCatching { output.delete() }
            }
        }
        if (!output.isFile || output.length() <= 0) {
            error("El muxer no generó el audiolibro")
        }
        // `title`, `artist` y `metadataJson` se reservan para escribir metadatos
        // MP4 en una versión posterior; el manifiesto de Kora ya los conserva.
    }

    private fun parseChapters(raw: String): List<Chapter> {
        if (raw.isBlank()) return emptyList()
        val array = JSONArray(raw)
        return (0 until array.length()).mapNotNull { index ->
            val item = array.optJSONObject(index) ?: return@mapNotNull null
            val path = item.optString("path").trim()
            if (path.isEmpty()) return@mapNotNull null
            Chapter(
                path = path,
                title = item.optString("title").ifBlank { "Capítulo ${index + 1}" },
            )
        }
    }

    /**
     * Lector mínimo de WAV PCM 16-bit mono a 24 kHz. Los archivos los escribe
     * `hound` en Rust con un encabezado RIFF estándar.
     */
    private class WavPcmReader(path: String) : Closeable {
        private val input = FileInputStream(File(path))
        private var remaining = 0L

        init {
            val riff = readFully(12)
            if (String(riff, 0, 4) != "RIFF" || String(riff, 8, 4) != "WAVE") {
                close()
                error("WAV inválido: $path")
            }
            while (true) {
                val header = readFully(8)
                val id = String(header, 0, 4)
                val size = readUInt32(header, 4)
                when (id) {
                    "fmt " -> {
                        val fmt = readFully(size.toInt())
                        val audioFormat = readUInt16(fmt, 0)
                        val channels = readUInt16(fmt, 2)
                        val sampleRate = readUInt32(fmt, 4).toInt()
                        val bits = readUInt16(fmt, 14)
                        if (audioFormat != 1 || channels != CHANNELS ||
                            sampleRate != SAMPLE_RATE || bits != 16
                        ) {
                            close()
                            error(
                                "WAV no compatible: formato=$audioFormat canales=$channels " +
                                    "frecuencia=$sampleRate bits=$bits",
                            )
                        }
                        if (size % 2L == 1L) skipFully(1)
                    }

                    "data" -> {
                        remaining = size
                        break
                    }

                    else -> skipFully(size + (size and 1L))
                }
            }
        }

        fun read(buffer: ByteArray): Int {
            if (remaining <= 0) return -1
            val toRead = minOf(buffer.size.toLong(), remaining).toInt()
            val read = input.read(buffer, 0, toRead)
            if (read > 0) remaining -= read
            return read
        }

        override fun close() {
            runCatching { input.close() }
        }

        private fun readFully(size: Int): ByteArray {
            if (size < 0 || size > 16 * 1024 * 1024) error("Bloque WAV inválido")
            val bytes = ByteArray(size)
            var offset = 0
            while (offset < size) {
                val read = input.read(bytes, offset, size - offset)
                if (read < 0) error("WAV truncado")
                offset += read
            }
            return bytes
        }

        private fun skipFully(size: Long) {
            var left = size
            while (left > 0) {
                val skipped = input.skip(left)
                if (skipped > 0) {
                    left -= skipped
                } else {
                    if (input.read() < 0) error("WAV truncado")
                    left -= 1
                }
            }
        }

        private fun readUInt16(bytes: ByteArray, offset: Int): Int =
            (bytes[offset].toInt() and 0xff) or
                ((bytes[offset + 1].toInt() and 0xff) shl 8)

        private fun readUInt32(bytes: ByteArray, offset: Int): Long =
            (bytes[offset].toLong() and 0xffL) or
                ((bytes[offset + 1].toLong() and 0xffL) shl 8) or
                ((bytes[offset + 2].toLong() and 0xffL) shl 16) or
                ((bytes[offset + 3].toLong() and 0xffL) shl 24)
    }
}
