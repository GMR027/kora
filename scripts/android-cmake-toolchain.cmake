# Toolchain de Android para dependencias CMake (whisper.cpp).
#
# Cargo compila cada ABI en un proceso distinto y expone el target en la
# variable TARGET. Este archivo traduce ese target a la ABI que espera el NDK
# y delega en el toolchain oficial. Así funciona tanto en builds arm64 como en
# los universales de release.

if(DEFINED ENV{ANDROID_NDK_HOME})
  set(_kora_ndk "$ENV{ANDROID_NDK_HOME}")
elseif(DEFINED ENV{NDK_HOME})
  set(_kora_ndk "$ENV{NDK_HOME}")
else()
  message(FATAL_ERROR "Kora: define ANDROID_NDK_HOME o NDK_HOME para CMake")
endif()

if(NOT CMAKE_ANDROID_ARCH_ABI)
  if(DEFINED ENV{TARGET})
    if("$ENV{TARGET}" MATCHES "^aarch64")
      set(CMAKE_ANDROID_ARCH_ABI "arm64-v8a")
    elseif("$ENV{TARGET}" MATCHES "^armv7")
      set(CMAKE_ANDROID_ARCH_ABI "armeabi-v7a")
    elseif("$ENV{TARGET}" MATCHES "^i686")
      set(CMAKE_ANDROID_ARCH_ABI "x86")
    elseif("$ENV{TARGET}" MATCHES "^x86_64")
      set(CMAKE_ANDROID_ARCH_ABI "x86_64")
    endif()
  endif()
endif()

if(NOT CMAKE_ANDROID_ARCH_ABI)
  set(CMAKE_ANDROID_ARCH_ABI "arm64-v8a")
endif()

# El toolchain legacy del NDK lee ANDROID_ABI, no CMAKE_ANDROID_ARCH_ABI.
set(ANDROID_ABI "${CMAKE_ANDROID_ARCH_ABI}")

set(CMAKE_ANDROID_API 24)
set(ANDROID_PLATFORM "android-24")

include("${_kora_ndk}/build/cmake/android.toolchain.cmake")
