cmake_minimum_required(VERSION 3.24)
project(Harmos VERSION 0.1.14 LANGUAGES C CXX)

find_package(Protobuf CONFIG REQUIRED)
find_package(gRPC CONFIG REQUIRED)
find_package(OpenSSL REQUIRED)
include(GNUInstallDirs)
include(CMakePackageConfigHelpers)
include(cmake/Protocol.cmake)

add_library(harmos_cpp_protocol STATIC)
harmos_protocol(harmos_cpp_protocol "${CMAKE_CURRENT_SOURCE_DIR}/../../proto"
    "${CMAKE_CURRENT_BINARY_DIR}/generated" ${HARMOS_PROTOCOLS})
set_target_properties(harmos_cpp_protocol PROPERTIES EXPORT_NAME Protocol)

add_library(harmos_cpp STATIC src/catalog.cpp src/context.cpp src/server.cpp src/generated/host.cpp)
add_library(Harmos::Cpp ALIAS harmos_cpp)
set_target_properties(harmos_cpp PROPERTIES EXPORT_NAME Cpp)
target_compile_features(harmos_cpp PUBLIC cxx_std_20)
target_include_directories(harmos_cpp PUBLIC
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/include>
    $<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}>)
target_link_libraries(harmos_cpp PUBLIC harmos_cpp_protocol PRIVATE OpenSSL::Crypto)
if(MSVC)
    target_compile_options(harmos_cpp PRIVATE /W4 /WX /EHsc /Zc:preprocessor)
else()
    target_compile_options(harmos_cpp PRIVATE -Wall -Wextra -Werror)
endif()

install(TARGETS harmos_cpp harmos_cpp_protocol EXPORT HarmosTargets
    ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR})
install(DIRECTORY include/ DESTINATION ${CMAKE_INSTALL_INCLUDEDIR})
foreach(protocol IN LISTS HARMOS_PROTOCOLS)
    get_filename_component(directory "${protocol}" DIRECTORY)
    string(REPLACE ".proto" ".pb.h" message_header "${protocol}")
    string(REPLACE ".proto" ".grpc.pb.h" service_header "${protocol}")
    install(FILES "${CMAKE_CURRENT_BINARY_DIR}/generated/${message_header}"
        "${CMAKE_CURRENT_BINARY_DIR}/generated/${service_header}"
        DESTINATION "${CMAKE_INSTALL_INCLUDEDIR}/harmos/detail/protocol/${directory}")
endforeach()
configure_package_config_file(cmake/HarmosConfig.cmake.in
    "${CMAKE_CURRENT_BINARY_DIR}/HarmosConfig.cmake"
    INSTALL_DESTINATION "${CMAKE_INSTALL_LIBDIR}/cmake/Harmos")
write_basic_package_version_file("${CMAKE_CURRENT_BINARY_DIR}/HarmosConfigVersion.cmake"
    VERSION ${PROJECT_VERSION} COMPATIBILITY SameMajorVersion)
install(EXPORT HarmosTargets NAMESPACE Harmos:: DESTINATION "${CMAKE_INSTALL_LIBDIR}/cmake/Harmos")
install(FILES "${CMAKE_CURRENT_BINARY_DIR}/HarmosConfig.cmake"
    "${CMAKE_CURRENT_BINARY_DIR}/HarmosConfigVersion.cmake"
    DESTINATION "${CMAKE_INSTALL_LIBDIR}/cmake/Harmos")

option(HARMOS_BUILD_TESTS "Build SDK Unit Tests and the Conformance Plugin" OFF)
if(HARMOS_BUILD_TESTS)
    enable_testing()
    add_subdirectory(tests)
    add_subdirectory(examples/conformance)
endif()

add_subdirectory(examples/client)
add_subdirectory(examples/plugin)
