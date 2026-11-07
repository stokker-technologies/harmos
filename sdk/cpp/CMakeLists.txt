cmake_minimum_required(VERSION 3.24)
project(HarmosCppPrototype VERSION 0.1.0 LANGUAGES C CXX)

include(FetchContent)
include(GNUInstallDirs)
include(CMakePackageConfigHelpers)

set(NANOARROW_INSTALL_SHARED OFF CACHE BOOL "Prototype Links Static nanoarrow" FORCE)
set(JSON_Install ON CACHE BOOL "Install the Prototype Header Dependency" FORCE)

FetchContent_Declare(nanoarrow
    URL https://github.com/apache/arrow-nanoarrow/archive/refs/tags/apache-arrow-nanoarrow-0.7.0.tar.gz
    URL_HASH SHA256=bb422ce7ed486abd95eb027a1ac092bbc1b5ed44955e89c098f0a1cb75109d5c)
FetchContent_Declare(nlohmann_json
    URL https://github.com/nlohmann/json/releases/download/v3.12.0/json.tar.xz
    URL_HASH SHA256=42f6e95cad6ec532fd372391373363b62a14af6d771056dbfc86160e6dfff7aa)
FetchContent_MakeAvailable(nanoarrow nlohmann_json)

set(HARMOS_FFI_LIBRARY "" CACHE FILEPATH "Task-Built Harmos Rust Static Library")
if(NOT EXISTS "${HARMOS_FFI_LIBRARY}")
    message(FATAL_ERROR "Run mise run build:cpp to build the Rust ABI first")
endif()

add_library(harmos_ffi STATIC IMPORTED GLOBAL)
set_target_properties(harmos_ffi PROPERTIES IMPORTED_LOCATION "${HARMOS_FFI_LIBRARY}")
add_library(harmos_cpp INTERFACE)
add_library(Harmos::Cpp ALIAS harmos_cpp)
set_target_properties(harmos_cpp PROPERTIES EXPORT_NAME Cpp)
target_compile_features(harmos_cpp INTERFACE cxx_std_20)
target_include_directories(harmos_cpp INTERFACE
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/../../crates/harmos-ffi/include>
    $<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}>)
target_link_libraries(harmos_cpp INTERFACE harmos_ffi nanoarrow_static nlohmann_json::nlohmann_json)
if(MSVC)
    target_compile_options(harmos_cpp INTERFACE /Zc:preprocessor /EHsc)
    target_link_options(harmos_cpp INTERFACE /OPT:REF /OPT:ICF /INCREMENTAL:NO)
    target_link_libraries(harmos_cpp INTERFACE ws2_32 userenv bcrypt ntdll advapi32)
else()
    target_link_libraries(harmos_cpp INTERFACE pthread dl m)
endif()

add_executable(harmos-bench examples/bench/main.cpp)
target_link_libraries(harmos-bench PRIVATE Harmos::Cpp)

install(DIRECTORY include/ DESTINATION ${CMAKE_INSTALL_INCLUDEDIR})
install(FILES ../../crates/harmos-ffi/include/harmos.h DESTINATION ${CMAKE_INSTALL_INCLUDEDIR})
install(FILES "${HARMOS_FFI_LIBRARY}" DESTINATION ${CMAKE_INSTALL_LIBDIR})
configure_package_config_file(cmake/HarmosConfig.cmake.in
    ${CMAKE_CURRENT_BINARY_DIR}/HarmosConfig.cmake
    INSTALL_DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/Harmos)
write_basic_package_version_file(${CMAKE_CURRENT_BINARY_DIR}/HarmosConfigVersion.cmake
    COMPATIBILITY SameMajorVersion)
install(FILES ${CMAKE_CURRENT_BINARY_DIR}/HarmosConfig.cmake
    ${CMAKE_CURRENT_BINARY_DIR}/HarmosConfigVersion.cmake DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/Harmos)
