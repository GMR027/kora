package dev.kora.reader.audio

import android.media.AudioFormat
import android.media.MediaCodec
import android.media.MediaExtractor
import android.media.MediaFormat
import java.io.File
import java.io.FileOutputStream
import java.io.RandomAccessFile
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.ceil
import kotlin.math.min

/**
 * Decodifica un rango de un archivo de audio a un WAV mono de 16 kHz.
 *
 * Android no incluye ffmpeg, así que usamos MediaExtractor/MediaCodec. Whisper
 * espera PCM float a 16 kHz; el WAV se escribe como PCM entero de 16 bits
 * porque después lo vuelve a leer el decodificador de Kora en Rust.
 */
internal object AudioRangeDecoder {
    private const val TARGET_RATE = 16_000
    private const val START_TOLERANCE_US = 0L

    data class Result(
        val durationMs: Long,
        val frames: Long,
        val sampleRate: Int,
    )

    fun decodeToWav(
        inputPath: String,
        outputPath: String,
        startMs: Long,
        durationMs: Long,
    ): Result {
        val input = File(inputPath)
        require(input.isFile) { "no se encontró el audio: $inputPath" }
        val output = File(outputPath)
        output.parentFile?.mkdirs()

        val extractor = MediaExtractor()
        var codec: MediaCodec? = null
        val writer = WavWriter(output, TARGET_RATE)
        try {
            extractor.setDataSource(input.absolutePath)
            val trackIndex = (0 until extractor.trackCount).firstOrNull { index ->
                extractor.getTrackFormat(index)
                    .getString(MediaFormat.KEY_MIME)
                    ?.startsWith("audio/") == true
            } ?: error("el archivo no contiene una pista de audio compatible")

            val format = extractor.getTrackFormat(trackIndex)
            val mime = format.getString(MediaFormat.KEY_MIME)
                ?: error("el audio no declara un formato MIME")
            extractor.selectTrack(trackIndex)

            val startUs = (startMs.coerceAtLeast(0)) * 1_000L
            val endUs = startUs + (durationMs.coerceAtLeast(1)) * 1_000L
            if (startUs > 0) {
                extractor.seekTo(startUs, MediaExtractor.SEEK_TO_PREVIOUS_SYNC)
            }

            codec = MediaCodec.createDecoderByType(mime)
            codec.configure(format, null, null, 0)
            codec.start()

            val info = MediaCodec.BufferInfo()
            var inputDone = false
            var outputDone = false
            var channelCount = format.intOr(MediaFormat.KEY_CHANNEL_COUNT, 2)
            var sampleRate = format.intOr(MediaFormat.KEY_SAMPLE_RATE, 48_000)
            var pcmEncoding = format.intOr(
                MediaFormat.KEY_PCM_ENCODING,
                AudioFormat.ENCODING_PCM_16BIT,
            )
            val resampler = StreamResampler(TARGET_RATE)
            var lastTimestampUs = -1L
            var idleLoops = 0

            while (!outputDone) {
                if (!inputDone) {
                    val inIndex = codec.dequeueInputBuffer(10_000)
                    if (inIndex >= 0) {
                        val buffer = codec.getInputBuffer(inIndex)
                        if (buffer == null) {
                            inputDone = true
                            codec.queueInputBuffer(
                                inIndex,
                                0,
                                0,
                                0,
                                MediaCodec.BUFFER_FLAG_END_OF_STREAM,
                            )
                        } else {
                            val size = extractor.readSampleData(buffer, 0)
                            if (size < 0) {
                                codec.queueInputBuffer(
                                    inIndex,
                                    0,
                                    0,
                                    0,
                                    MediaCodec.BUFFER_FLAG_END_OF_STREAM,
                                )
                                inputDone = true
                            } else {
                                codec.queueInputBuffer(
                                    inIndex,
                                    0,
                                    size,
                                    extractor.sampleTime,
                                    0,
                                )
                                extractor.advance()
                            }
                        }
                    }
                }

                when (val outIndex = codec.dequeueOutputBuffer(info, 10_000)) {
                    MediaCodec.INFO_OUTPUT_FORMAT_CHANGED -> {
                        val changed = codec.outputFormat
                        channelCount = changed.intOr(MediaFormat.KEY_CHANNEL_COUNT, channelCount)
                        sampleRate = changed.intOr(MediaFormat.KEY_SAMPLE_RATE, sampleRate)
                        pcmEncoding = changed.intOr(MediaFormat.KEY_PCM_ENCODING, pcmEncoding)
                        resampler.reset()
                    }

                    MediaCodec.INFO_TRY_AGAIN_LATER -> {
                        idleLoops += 1
                        if (inputDone && idleLoops > 200) {
                            outputDone = true
                        }
                    }

                    else -> {
                        idleLoops = 0
                        if (outIndex >= 0) {
                            if (info.size > 0) {
                                val timestampUs =
                                    if (info.presentationTimeUs >= 0) {
                                        info.presentationTimeUs
                                    } else {
                                        lastTimestampUs
                                    }
                                if (timestampUs >= 0) {
                                    lastTimestampUs = timestampUs
                                }
                                val buffer = codec.getOutputBuffer(outIndex)
                                if (buffer != null) {
                                    buffer.position(info.offset)
                                    buffer.limit(info.offset + info.size)
                                    val mono = readMono(buffer, pcmEncoding, channelCount)
                                    val trimmed = trimRange(
                                        mono,
                                        sampleRate,
                                        timestampUs,
                                        startUs,
                                        endUs,
                                    )
                                    if (trimmed.isNotEmpty()) {
                                        resampler.process(trimmed, sampleRate) { chunk ->
                                            writer.write(chunk)
                                        }
                                    }
                                }
                            }
                            codec.releaseOutputBuffer(outIndex, false)
                            if (info.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0) {
                                outputDone = true
                            }
                        }
                    }
                }
            }

            val frames = writer.finish()
            if (frames <= 0) {
                error("el decodificador no produjo audio para el rango solicitado")
            }
            return Result(
                durationMs = (frames * 1_000L / TARGET_RATE).coerceAtLeast(1),
                frames = frames,
                sampleRate = TARGET_RATE,
            )
        } finally {
            runCatching { codec?.stop() }
            runCatching { codec?.release() }
            runCatching { extractor.release() }
            runCatching { writer.abort() }
        }
    }

    private fun trimRange(
        samples: FloatArray,
        sampleRate: Int,
        timestampUs: Long,
        startUs: Long,
        endUs: Long,
    ): FloatArray {
        if (samples.isEmpty()) return samples
        if (timestampUs < 0) return samples
        val usPerSample = 1_000_000.0 / sampleRate.toDouble()
        val first = ceil((startUs - START_TOLERANCE_US - timestampUs) / usPerSample)
            .toInt()
            .coerceAtLeast(0)
        val last = ceil((endUs - timestampUs) / usPerSample)
            .toInt()
            .coerceIn(0, samples.size)
        if (last <= first) return FloatArray(0)
        return samples.copyOfRange(first, last)
    }

    private fun readMono(
        buffer: ByteBuffer,
        encoding: Int,
        channels: Int,
    ): FloatArray {
        val safeChannels = channels.coerceAtLeast(1)
        buffer.order(ByteOrder.nativeOrder())
        return when (encoding) {
            AudioFormat.ENCODING_PCM_FLOAT -> {
                val floats = buffer.asFloatBuffer()
                val frames = floats.remaining() / safeChannels
                FloatArray(frames) { frame ->
                    var sum = 0f
                    for (channel in 0 until safeChannels) {
                        sum += floats.get(frame * safeChannels + channel)
                    }
                    sum / safeChannels
                }
            }

            AudioFormat.ENCODING_PCM_16BIT -> {
                val shorts = buffer.asShortBuffer()
                val frames = shorts.remaining() / safeChannels
                FloatArray(frames) { frame ->
                    var sum = 0f
                    for (channel in 0 until safeChannels) {
                        sum += shorts.get(frame * safeChannels + channel) / 32768f
                    }
                    sum / safeChannels
                }
            }

            else -> error(
                "formato PCM no soportado por el decodificador: $encoding " +
                    "(se esperaba 16 bits o float)",
            )
        }
    }

    private fun MediaFormat.intOr(key: String, fallback: Int): Int =
        if (containsKey(key)) getInteger(key) else fallback

    /**
     * Remuestreador lineal con estado entre buffers. Para voz y Whisper el
     * resultado es suficiente y evita cargar todo el capítulo en memoria.
     */
    private class StreamResampler(private val outputRate: Int) {
        private var inputRate = 0
        private var position = 0.0
        private var previous = 0f

        fun reset() {
            inputRate = 0
            position = 0.0
            previous = 0f
        }

        fun process(
            input: FloatArray,
            rate: Int,
            sink: (FloatArray) -> Unit,
        ) {
            if (input.isEmpty() || rate <= 0) return
            if (rate != inputRate) {
                inputRate = rate
                position = 0.0
                previous = input[0]
            }
            val step = rate.toDouble() / outputRate.toDouble()
            val output = ArrayList<Float>(((input.size / step).toInt() + 2).coerceAtLeast(1))
            var cursor = position
            while (cursor < input.size) {
                val index = cursor.toInt()
                val fraction = cursor - index
                val current = input[min(index, input.size - 1)]
                val before = if (index == 0) previous else input[index - 1]
                output.add(before + (current - before) * fraction.toFloat())
                cursor += step
            }
            previous = input[input.size - 1]
            position = cursor - input.size
            if (output.isNotEmpty()) {
                sink(output.toFloatArray())
            }
        }
    }

    /** Escribe un WAV PCM 16 bits mono 16 kHz con cabecera diferida. */
    private class WavWriter(file: File, private val sampleRate: Int) {
        private val file = file
        private val stream = FileOutputStream(file)
        private var dataBytes = 0L
        private var finished = false

        init {
            stream.write(ByteArray(44))
        }

        fun write(samples: FloatArray) {
            if (samples.isEmpty()) return
            val bytes = ByteArray(samples.size * 2)
            var offset = 0
            for (sample in samples) {
                val value = (sample.coerceIn(-1f, 1f) * 32767f).toInt()
                bytes[offset] = (value and 0xFF).toByte()
                bytes[offset + 1] = ((value shr 8) and 0xFF).toByte()
                offset += 2
            }
            stream.write(bytes)
            dataBytes += bytes.size
        }

        fun finish(): Long {
            stream.flush()
            stream.close()
            val header = header(dataBytes)
            RandomAccessFile(file, "rw").use { output ->
                output.seek(0)
                output.write(header)
            }
            finished = true
            return dataBytes / 2
        }

        fun abort() {
            if (finished) return
            runCatching { stream.close() }
        }

        private fun header(dataBytes: Long): ByteArray {
            val buffer = ByteBuffer.allocate(44).order(ByteOrder.LITTLE_ENDIAN)
            buffer.put("RIFF".toByteArray(Charsets.US_ASCII))
            buffer.putInt((36 + dataBytes).toInt())
            buffer.put("WAVE".toByteArray(Charsets.US_ASCII))
            buffer.put("fmt ".toByteArray(Charsets.US_ASCII))
            buffer.putInt(16)
            buffer.putShort(1)
            buffer.putShort(1)
            buffer.putInt(sampleRate)
            buffer.putInt(sampleRate * 2)
            buffer.putShort(2)
            buffer.putShort(16)
            buffer.put("data".toByteArray(Charsets.US_ASCII))
            buffer.putInt(dataBytes.toInt())
            return buffer.array()
        }
    }
}
