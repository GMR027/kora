package dev.kora.reader.tts

import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.LinearGradient
import android.graphics.Paint
import android.graphics.Path
import android.graphics.Shader
import android.graphics.Typeface
import android.text.Layout
import android.text.StaticLayout
import android.text.TextPaint
import java.io.ByteArrayOutputStream

/**
 * Portada de respaldo para la tarjeta de reproducción cuando el libro no tiene
 * una. Sigue el lenguaje visual de Kora: papel crema, tinta cálida y el sello
 * editorial en siena.
 */
object DefaultTtsArtwork {
    private const val SIZE = 512
    private const val INK = "#1B1C1A"
    private const val MUTED = "#6E5A4D"
    private const val SIENNA = "#8D4B00"
    private const val PAPER = "#FBF9F5"
    private const val PAPER_DEEP = "#EFE4D4"
    private const val LINE = "#E4E2DE"

    fun render(title: String, author: String?): ByteArray {
        val bitmap = Bitmap.createBitmap(SIZE, SIZE, Bitmap.Config.ARGB_8888)
        val canvas = Canvas(bitmap)
        val safeTitle = title.trim().ifEmpty { "Kora" }
        val safeAuthor = author?.trim().orEmpty()

        val background = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            shader = LinearGradient(
                0f,
                0f,
                0f,
                SIZE.toFloat(),
                Color.parseColor(PAPER),
                Color.parseColor(PAPER_DEEP),
                Shader.TileMode.CLAMP,
            )
        }
        canvas.drawRect(0f, 0f, SIZE.toFloat(), SIZE.toFloat(), background)

        val border = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            style = Paint.Style.STROKE
            strokeWidth = 1.5f
            color = Color.parseColor(LINE)
        }
        canvas.drawRect(18f, 18f, SIZE - 18f, SIZE - 18f, border)

        val label = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.parseColor(MUTED)
            textSize = 20f
            typeface = Typeface.create("sans-serif-medium", Typeface.NORMAL)
            textAlign = Paint.Align.CENTER
        }
        canvas.drawText("KORA", SIZE / 2f, 74f, label)

        val kicker = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.parseColor(MUTED)
            textSize = 13f
            typeface = Typeface.create("sans-serif", Typeface.NORMAL)
            textAlign = Paint.Align.CENTER
        }
        canvas.drawText("LECTURA EN VOZ ALTA", SIZE / 2f, 98f, kicker)

        drawMark(canvas)

        val titlePaint = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.parseColor(INK)
            textSize = 46f
            typeface = Typeface.create("serif", Typeface.NORMAL)
            textAlign = Paint.Align.CENTER
        }
        val titleLayout = StaticLayout.Builder
            .obtain(safeTitle, 0, safeTitle.length, titlePaint, 380)
            .setAlignment(Layout.Alignment.ALIGN_CENTER)
            .setLineSpacing(4f, 1f)
            .setMaxLines(4)
            .setEllipsize(android.text.TextUtils.TruncateAt.END)
            .build()
        canvas.save()
        canvas.translate((SIZE - 380) / 2f, 214f)
        titleLayout.draw(canvas)
        canvas.restore()

        val rule = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.parseColor(LINE)
            strokeWidth = 1.5f
        }
        canvas.drawLine(196f, 420f, 316f, 420f, rule)

        if (safeAuthor.isNotEmpty()) {
            val authorPaint = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
                color = Color.parseColor(MUTED)
                textSize = 22f
                typeface = Typeface.create("sans-serif", Typeface.NORMAL)
                textAlign = Paint.Align.CENTER
            }
            val authorLayout = StaticLayout.Builder
                .obtain(safeAuthor, 0, safeAuthor.length, authorPaint, 360)
                .setAlignment(Layout.Alignment.ALIGN_CENTER)
                .setMaxLines(1)
                .setEllipsize(android.text.TextUtils.TruncateAt.END)
                .build()
            canvas.save()
            canvas.translate((SIZE - 360) / 2f, 446f)
            authorLayout.draw(canvas)
            canvas.restore()
        }

        val footer = TextPaint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.parseColor(MUTED)
            textSize = 12f
            typeface = Typeface.create("sans-serif", Typeface.NORMAL)
            textAlign = Paint.Align.CENTER
        }
        canvas.drawText("KORA · LECTURA", SIZE / 2f, 486f, footer)

        val output = ByteArrayOutputStream()
        bitmap.compress(Bitmap.CompressFormat.PNG, 100, output)
        bitmap.recycle()
        return output.toByteArray()
    }

    private fun drawMark(canvas: Canvas) {
        val mark = Paint(Paint.ANTI_ALIAS_FLAG).apply {
            color = Color.parseColor(SIENNA)
            style = Paint.Style.STROKE
            strokeWidth = 3.5f
            strokeCap = Paint.Cap.ROUND
            strokeJoin = Paint.Join.ROUND
        }
        val stem = Paint(mark).apply { style = Paint.Style.STROKE }
        canvas.drawLine(256f, 132f, 256f, 178f, stem)

        val left = Path().apply {
            moveTo(254f, 146f)
            cubicTo(228f, 136f, 214f, 154f, 222f, 174f)
            cubicTo(238f, 176f, 250f, 164f, 254f, 146f)
            close()
        }
        val right = Path().apply {
            moveTo(258f, 146f)
            cubicTo(284f, 136f, 298f, 154f, 290f, 174f)
            cubicTo(274f, 176f, 262f, 164f, 258f, 146f)
            close()
        }
        canvas.drawPath(left, mark)
        canvas.drawPath(right, mark)
    }
}