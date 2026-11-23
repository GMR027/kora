package dev.kora.reader

import android.app.DownloadManager
import android.content.ActivityNotFoundException
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.graphics.Color
import android.media.MediaMetadataRetriever
import android.net.Uri
import android.net.wifi.WifiManager
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.OpenableColumns
import android.provider.Settings
import android.view.ViewGroup
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.activity.enableEdgeToEdge
import androidx.core.content.ContextCompat
import androidx.core.content.FileProvider
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import java.io.File
import java.security.MessageDigest
import java.util.UUID

class MainActivity : TauriActivity() {
  private var multicastLock: WifiManager.MulticastLock? = null
  private val chromeBridge = ChromeBridge()
  private var webView: WebView? = null
  private var pendingDownloadId: Long = -1
  private var pendingSha256: String = ""
  private var downloadReceiver: BroadcastReceiver? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)

    // Android 15+ dibuja la app de borde a borde. El WebView no recibe los
    // insets del sistema en env(safe-area-inset-*), así que reservamos aquí
    // el espacio de la barra de estado, la barra de navegación y los recortes
    // de pantalla. Sin esto, el encabezado queda debajo del reloj y los
    // botones de la parte superior e inferior reciben toques del sistema.
    val content = findViewById<ViewGroup>(android.R.id.content)
    ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
      val bars = insets.getInsets(
        WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
      )
      view.setPadding(bars.left, bars.top, bars.right, bars.bottom)
      insets
    }
    ViewCompat.requestApplyInsets(content)

    // Fondo e iconos de sistema por defecto (tema claro). El frontend los
    // actualiza al tema real mediante window.KoraChrome.setChrome().
    applyChrome("#fbf9f5", false)
  }

  override fun onWebViewCreate(webView: WebView) {
    super.onWebViewCreate(webView)
    this.webView = webView
    webView.addJavascriptInterface(chromeBridge, "KoraChrome")
  }

  override fun onResume() {
    super.onResume()
    // mDNS utiliza multicast. Android filtra esos paquetes si la app no
    // mantiene un MulticastLock, lo que impide descubrir peers de sync.
    // Solo lo sostenemos mientras la app está visible para no castigar la
    // batería en segundo plano; el transporte QUIC sigue escuchando.
    if (multicastLock == null) {
      val wifi = applicationContext.getSystemService(Context.WIFI_SERVICE) as WifiManager
      multicastLock = wifi.createMulticastLock("kora-mdns").apply {
        setReferenceCounted(false)
        acquire()
      }
    }
  }

  override fun onPause() {
    multicastLock?.release()
    multicastLock = null
    super.onPause()
  }

  override fun onDestroy() {
    downloadReceiver?.let { receiver ->
      runCatching { unregisterReceiver(receiver) }
    }
    downloadReceiver = null
    super.onDestroy()
  }

  private fun applyChrome(background: String, dark: Boolean) {
    try {
      window.decorView.setBackgroundColor(Color.parseColor(background))
    } catch (_: IllegalArgumentException) {
      window.decorView.setBackgroundColor(Color.parseColor("#fbf9f5"))
    }
    val controller = WindowCompat.getInsetsController(window, window.decorView)
    controller.isAppearanceLightStatusBars = !dark
    controller.isAppearanceLightNavigationBars = !dark
  }

  private fun emitUpdateEvent(name: String, message: String? = null) {
    val view = webView ?: return
    val detail = if (message != null) {
      "{\"message\":${org.json.JSONObject.quote(message)}}"
    } else {
      "{}"
    }
    view.post {
      view.evaluateJavascript(
        "window.dispatchEvent(new CustomEvent('$name',{detail:$detail}))",
        null
      )
    }
  }

  private fun startUpdateDownload(url: String, sha256: String, fileName: String) {
    if (!url.startsWith("https://") && !url.startsWith("http://")) {
      emitUpdateEvent("kora:update-error", "URL de actualización inválida")
      return
    }
    val manager = getSystemService(Context.DOWNLOAD_SERVICE) as DownloadManager
    if (pendingDownloadId != -1L) {
      runCatching { manager.remove(pendingDownloadId) }
      pendingDownloadId = -1
    }

    val safeName = fileName
      .replace(Regex("[^A-Za-z0-9._-]"), "-")
      .ifBlank { "kora-update.apk" }
      .let { if (it.endsWith(".apk", ignoreCase = true)) it else "$it.apk" }
    val updateDir = File(
      getExternalFilesDir(Environment.DIRECTORY_DOWNLOADS),
      "updates",
    )
    updateDir.mkdirs()
    updateDir.listFiles()
      ?.filter { it.name.startsWith("kora-") && it.name.endsWith(".apk") }
      ?.forEach { it.delete() }
    File(updateDir, safeName).takeIf { it.exists() }?.delete()

    registerDownloadReceiver()
    pendingSha256 = sha256.trim()
    val request = DownloadManager.Request(Uri.parse(url)).apply {
      setTitle("Kora")
      setDescription("Actualización de Kora")
      setMimeType("application/vnd.android.package-archive")
      setNotificationVisibility(DownloadManager.Request.VISIBILITY_VISIBLE_NOTIFY_COMPLETED)
      setDestinationInExternalFilesDir(
        this@MainActivity,
        Environment.DIRECTORY_DOWNLOADS,
        "updates/$safeName",
      )
      setAllowedOverMetered(true)
      setAllowedOverRoaming(false)
    }

    try {
      pendingDownloadId = manager.enqueue(request)
    } catch (error: Exception) {
      emitUpdateEvent(
        "kora:update-error",
        error.message ?: "No se pudo iniciar la descarga",
      )
    }
  }

  private fun registerDownloadReceiver() {
    if (downloadReceiver != null) return
    val receiver = object : BroadcastReceiver() {
      override fun onReceive(context: Context?, intent: Intent?) {
        val id = intent?.getLongExtra(DownloadManager.EXTRA_DOWNLOAD_ID, -1L) ?: -1L
        if (id == pendingDownloadId) verifyUpdateDownload(id)
      }
    }
    downloadReceiver = receiver
    ContextCompat.registerReceiver(
      this,
      receiver,
      IntentFilter(DownloadManager.ACTION_DOWNLOAD_COMPLETE),
      ContextCompat.RECEIVER_EXPORTED,
    )
  }

  private fun verifyUpdateDownload(id: Long) {
    val manager = getSystemService(Context.DOWNLOAD_SERVICE) as DownloadManager
    val cursor = manager.query(DownloadManager.Query().setFilterById(id))
    try {
      if (cursor == null || !cursor.moveToFirst()) {
        emitUpdateEvent("kora:update-error", "No se encontró la descarga")
        return
      }
      val status = cursor.getInt(
        cursor.getColumnIndexOrThrow(DownloadManager.COLUMN_STATUS),
      )
      if (status != DownloadManager.STATUS_SUCCESSFUL) {
        emitUpdateEvent(
          "kora:update-error",
          "La descarga de la actualización falló",
        )
        return
      }
      val local = cursor.getString(
        cursor.getColumnIndexOrThrow(DownloadManager.COLUMN_LOCAL_URI),
      )
      val file = local?.let { Uri.parse(it).path }?.let(::File)
      if (file == null || !file.exists()) {
        emitUpdateEvent("kora:update-error", "El APK descargado no existe")
        return
      }
      Thread {
        try {
          if (pendingSha256.isNotBlank()) {
            val actual = sha256(file)
            if (!actual.equals(pendingSha256, ignoreCase = true)) {
              emitUpdateEvent(
                "kora:update-error",
                "El APK descargado no coincide con el hash esperado",
              )
              return@Thread
            }
          }
          runOnUiThread {
            emitUpdateEvent("kora:update-ready")
            installUpdate(file)
          }
        } catch (error: Exception) {
          emitUpdateEvent(
            "kora:update-error",
            error.message ?: "No se pudo verificar el APK",
          )
        }
      }.start()
    } finally {
      cursor?.close()
    }
  }

  private fun sha256(file: File): String {
    val digest = MessageDigest.getInstance("SHA-256")
    file.inputStream().use { input ->
      val buffer = ByteArray(64 * 1024)
      while (true) {
        val read = input.read(buffer)
        if (read <= 0) break
        digest.update(buffer, 0, read)
      }
    }
    return digest.digest().joinToString("") { "%02x".format(it.toInt() and 0xff) }
  }

  private fun installUpdate(file: File) {
    if (
      Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
      !packageManager.canRequestPackageInstalls()
    ) {
      emitUpdateEvent("kora:update-permission")
      val settings = Intent(
        Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
        Uri.parse("package:$packageName"),
      ).apply { addFlags(Intent.FLAG_ACTIVITY_NEW_TASK) }
      startActivity(settings)
      return
    }

    val uri = FileProvider.getUriForFile(
      this,
      "$packageName.fileprovider",
      file,
    )
    val intent = Intent(Intent.ACTION_VIEW).apply {
      setDataAndType(uri, "application/vnd.android.package-archive")
      addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
      addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
    }
    try {
      startActivity(intent)
    } catch (error: ActivityNotFoundException) {
      emitUpdateEvent(
        "kora:update-error",
        error.message ?: "No se pudo abrir el instalador",
      )
    }
  }

  private fun importCacheRoot(): File = File(cacheDir, "imports").apply { mkdirs() }

  // El selector de Android entrega `content://`; Rust necesita una ruta real.
  // Copiamos el archivo a la caché conservando su nombre para que la
  // biblioteca pueda leerlo y mostrar el título correcto.
  private fun stageContentUri(uri: String): String {
    if (!uri.startsWith("content://")) return uri
    return try {
      val parsed = Uri.parse(uri)
      val rawName = queryDisplayName(parsed) ?: parsed.lastPathSegment ?: "libro"
      val extension = when (contentResolver.getType(parsed)) {
        "application/epub+zip" -> ".epub"
        "application/pdf" -> ".pdf"
        "audio/mpeg", "audio/mp3" -> ".mp3"
        "audio/mp4", "audio/x-m4a", "audio/m4a" -> ".m4a"
        "audio/x-m4b", "application/x-m4b" -> ".m4b"
        "audio/aac" -> ".aac"
        "audio/flac", "audio/x-flac" -> ".flac"
        "audio/ogg" -> ".ogg"
        "audio/opus" -> ".opus"
        "image/jpeg", "image/jpg", "image/pjpeg" -> ".jpg"
        "image/png" -> ".png"
        "image/webp" -> ".webp"
        else -> ""
      }
      var name = sanitizeFileName(rawName)
      val knownExtensions = listOf(
        ".epub", ".pdf", ".mp3", ".m4a", ".m4b", ".aac", ".flac", ".ogg", ".opus",
        ".jpg", ".jpeg", ".png", ".webp",
      )
      if (extension.isNotEmpty() && knownExtensions.none { name.endsWith(it, ignoreCase = true) }) {
        name += extension
      }

      val directory = File(importCacheRoot(), UUID.randomUUID().toString()).apply { mkdirs() }
      val target = File(directory, name)
      val input = contentResolver.openInputStream(parsed) ?: return ""
      input.use { source ->
        target.outputStream().use { output -> source.copyTo(output, 64 * 1024) }
      }
      target.absolutePath
    } catch (_: Exception) {
      ""
    }
  }

  private fun queryDisplayName(uri: Uri): String? {
    val projection = arrayOf(OpenableColumns.DISPLAY_NAME)
    contentResolver.query(uri, projection, null, null, null)?.use { cursor ->
      if (cursor.moveToFirst()) {
        val index = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
        if (index >= 0) {
          val value = cursor.getString(index)
          if (!value.isNullOrBlank()) return value
        }
      }
    }
    return null
  }

  private fun sanitizeFileName(value: String): String {
    val cleaned = value
      .replace(Regex("[\\\\/:*?\"<>|\\u0000-\\u001F]"), "-")
      .trim()
      .trim('.')
      .take(120)
    return cleaned.ifBlank { "libro-${System.currentTimeMillis()}" }
  }

  private fun probeAudioFile(path: String): String {
    val retriever = MediaMetadataRetriever()
    return try {
      retriever.setDataSource(path)
      val duration = retriever
        .extractMetadata(MediaMetadataRetriever.METADATA_KEY_DURATION)
        ?.toLongOrNull()
        ?: 0L
      if (duration <= 0L) return ""

      val coverPath = retriever.embeddedPicture?.let { bytes ->
        val extension = if (
          bytes.size >= 4 &&
          bytes[0] == 0x89.toByte() &&
          bytes[1] == 0x50.toByte() &&
          bytes[2] == 0x4E.toByte() &&
          bytes[3] == 0x47.toByte()
        ) "png" else "jpg"
        val directory = File(path).parentFile
          ?: File(importCacheRoot(), UUID.randomUUID().toString()).apply { mkdirs() }
        directory.mkdirs()
        val file = File(directory, "cover.$extension")
        file.writeBytes(bytes)
        file.absolutePath
      }

      org.json.JSONObject().apply {
        put("title", retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_TITLE))
        put(
          "artist",
          retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_ARTIST)
            ?: retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_ALBUMARTIST),
        )
        put("album", retriever.extractMetadata(MediaMetadataRetriever.METADATA_KEY_ALBUM))
        put("durationMs", duration)
        put("coverPath", coverPath)
        put("language", java.util.Locale.getDefault().language)
      }.toString()
    } catch (_: Exception) {
      ""
    } finally {
      runCatching { retriever.release() }
    }
  }

  inner class ChromeBridge {
    @JavascriptInterface
    fun setChrome(background: String, dark: Boolean) {
      runOnUiThread { applyChrome(background, dark) }
    }

    @JavascriptInterface
    fun downloadUpdate(url: String, sha256: String, fileName: String) {
      runOnUiThread { startUpdateDownload(url, sha256, fileName) }
    }

    @JavascriptInterface
    fun stageImportFile(uri: String): String = stageContentUri(uri)

    @JavascriptInterface
    fun probeAudio(path: String): String = probeAudioFile(path)

    @JavascriptInterface
    fun discardImportFile(path: String) {
      try {
        val root = importCacheRoot().canonicalFile
        val file = File(path).canonicalFile
        if (!file.path.startsWith(root.path + File.separator)) return
        file.parentFile?.deleteRecursively()
      } catch (_: Exception) {
        // La caché se limpia sola si el sistema necesita espacio.
      }
    }
  }
}
