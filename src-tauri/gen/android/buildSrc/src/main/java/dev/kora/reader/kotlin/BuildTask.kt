import java.io.File
import org.apache.tools.ant.taskdefs.condition.Os
import org.gradle.api.DefaultTask
import org.gradle.api.GradleException
import org.gradle.api.logging.LogLevel
import org.gradle.api.tasks.Input
import org.gradle.api.tasks.TaskAction
import javax.inject.Inject
import org.gradle.process.ExecOperations

abstract class BuildTask : DefaultTask() {
    @get:Inject
    abstract val execOperations: ExecOperations

    @Input
    var rootDirRel: String? = null
    @Input
    var projectDir: String? = null
    @Input
    var target: String? = null
    @Input
    var release: Boolean? = null

    @TaskAction
    fun assemble() {
        val executable = """pnpm""";
        try {
            runTauriCli(executable)
        } catch (e: Exception) {
            if (Os.isFamily(Os.FAMILY_WINDOWS)) {
                // Try different Windows-specific extensions
                val fallbacks = listOf(
                    "$executable.exe",
                    "$executable.cmd",
                    "$executable.bat",
                )

                var lastException: Exception = e
                for (fallback in fallbacks) {
                    try {
                        runTauriCli(fallback)
                        return
                    } catch (fallbackException: Exception) {
                        lastException = fallbackException
                    }
                }
                throw lastException
            } else {
                throw e;
            }
        }
    }

    fun runTauriCli(executable: String) {
        val rootDirRel = rootDirRel ?: throw GradleException("rootDirRel cannot be null")
        val target = target ?: throw GradleException("target cannot be null")
        val release = release ?: throw GradleException("release cannot be null")
        val args = listOf("tauri", "android", "android-studio-script");

        execOperations.exec {
            workingDir(File(projectDir, rootDirRel))
            executable(executable)
            args(args)
            if (logger.isEnabled(LogLevel.DEBUG)) {
                args("-vv")
            } else if (logger.isEnabled(LogLevel.INFO)) {
                args("-v")
            }
            if (release) {
                args("--release")
            }
            args(listOf("--target", target))

            // whisper-rs compila whisper.cpp con CMake. En Android hay que
            // apuntar CMake al toolchain del NDK y fijar la ABI del target.
            val ndk = findNdkHome()
            if (ndk != null) {
                environment("NDK_HOME", ndk)
                environment("ANDROID_NDK_HOME", ndk)
                environment(
                    "CMAKE_TOOLCHAIN_FILE",
                    File(
                        File(projectDir, rootDirRel),
                        "scripts/android-cmake-toolchain.cmake",
                    ).absolutePath,
                )
                environment("CMAKE_ANDROID_ARCH_ABI", abiForTarget(target))
                environment("CMAKE_ANDROID_API", "24")
            }
            // Usa los bindings empaquetados y evita depender de libclang al
            // compilar para Android.
            environment("WHISPER_DONT_GENERATE_BINDINGS", "1")
        }.assertNormalExitValue()
    }

    private fun abiForTarget(target: String): String = when (target) {
        "aarch64" -> "arm64-v8a"
        "armv7" -> "armeabi-v7a"
        "i686" -> "x86"
        "x86_64" -> "x86_64"
        else -> throw GradleException("ABI desconocida para el target $target")
    }

    private fun findNdkHome(): String? {
        val direct = listOf(
            System.getenv("NDK_HOME"),
            System.getenv("ANDROID_NDK_HOME"),
        ).firstOrNull { !it.isNullOrBlank() && File(it).isDirectory }
        if (direct != null) return direct

        val sdk = System.getenv("ANDROID_HOME")
            ?: System.getenv("ANDROID_SDK_ROOT")
            ?: return null
        val directory = File(sdk, "ndk")
        return directory.listFiles()
            ?.filter { it.isDirectory }
            ?.maxByOrNull { it.name }
            ?.absolutePath
    }
}