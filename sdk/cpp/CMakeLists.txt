cmake_minimum_required(VERSION 3.24)
project(HarmosCpp VERSION 0.1.0 LANGUAGES C CXX)
set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
include(cmake/Dependencies.cmake)

# Generate Protocol and Fixture Messages with the Same Pinned Compiler as the Runtime.
# Only Callback Server APIs Are Needed; Omit Unused Synchronous and CQ APIs.
set(HARMOS_PROTO_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../../proto")
set(HARMOS_GENERATED "${CMAKE_CURRENT_BINARY_DIR}/generated")
file(MAKE_DIRECTORY "${HARMOS_GENERATED}")
add_custom_command(
    OUTPUT "${HARMOS_GENERATED}/harmos/v1/plugin.pb.cc" "${HARMOS_GENERATED}/harmos/v1/plugin.pb.h"
        "${HARMOS_GENERATED}/harmos/v1/plugin.grpc.pb.cc" "${HARMOS_GENERATED}/harmos/v1/plugin.grpc.pb.h"
    COMMAND $<TARGET_FILE:protoc>
        --proto_path=${HARMOS_PROTO_ROOT}
        --proto_path=${grpc_SOURCE_DIR}/third_party/protobuf/src
        --cpp_out=${HARMOS_GENERATED}
        --grpc_out=allow_sync_server_api=false,allow_cq_api=false:${HARMOS_GENERATED}
        --plugin=protoc-gen-grpc=$<TARGET_FILE:grpc_cpp_plugin>
        "${HARMOS_PROTO_ROOT}/harmos/v1/plugin.proto"
    DEPENDS protoc grpc_cpp_plugin "${HARMOS_PROTO_ROOT}/harmos/v1/plugin.proto"
    VERBATIM)
add_library(harmos_cpp_protocol STATIC
    "${HARMOS_GENERATED}/harmos/v1/plugin.pb.cc" "${HARMOS_GENERATED}/harmos/v1/plugin.grpc.pb.cc")
target_include_directories(harmos_cpp_protocol SYSTEM PUBLIC "${HARMOS_GENERATED}")
target_link_libraries(harmos_cpp_protocol PUBLIC grpc++ libprotobuf)
add_library(harmos_cpp STATIC src/catalog.cpp src/context.cpp src/server.cpp)
add_library(Harmos::Cpp ALIAS harmos_cpp)
target_compile_features(harmos_cpp PUBLIC cxx_std_20)
target_include_directories(harmos_cpp PUBLIC include)
target_include_directories(harmos_cpp SYSTEM PUBLIC
    "${grpc_SOURCE_DIR}/include"
    "${grpc_SOURCE_DIR}/third_party/protobuf/src"
    "${grpc_SOURCE_DIR}/third_party/abseil-cpp")
target_link_libraries(harmos_cpp PUBLIC harmos_cpp_protocol PRIVATE crypto)
if(MSVC)
    target_compile_options(harmos_cpp PRIVATE /W4 /WX /EHsc /Zc:preprocessor)
else()
    target_compile_options(harmos_cpp PRIVATE -Wall -Wextra -Werror)
endif()

option(HARMOS_BUILD_TESTS "Build SDK Unit Tests and the Conformance Plugin" ${PROJECT_IS_TOP_LEVEL})
if(HARMOS_BUILD_TESTS)
    add_custom_command(
        OUTPUT "${HARMOS_GENERATED}/harmos/conformance/v1/fixture.pb.cc" "${HARMOS_GENERATED}/harmos/conformance/v1/fixture.pb.h"
        COMMAND $<TARGET_FILE:protoc> --proto_path=${CMAKE_CURRENT_SOURCE_DIR}/proto
            --proto_path=${grpc_SOURCE_DIR}/third_party/protobuf/src
            --cpp_out=${HARMOS_GENERATED} "${CMAKE_CURRENT_SOURCE_DIR}/proto/harmos/conformance/v1/fixture.proto"
        DEPENDS protoc "${CMAKE_CURRENT_SOURCE_DIR}/proto/harmos/conformance/v1/fixture.proto"
        VERBATIM)
    add_library(harmos_cpp_fixture STATIC "${HARMOS_GENERATED}/harmos/conformance/v1/fixture.pb.cc")
    target_link_libraries(harmos_cpp_fixture PUBLIC Harmos::Cpp)
    add_executable(harmos-cpp-conformance examples/conformance/main.cpp)
    target_link_libraries(harmos-cpp-conformance PRIVATE harmos_cpp_fixture)

    enable_testing()
    add_executable(harmos-cpp-tests tests/unit/catalog.cpp)
    target_link_libraries(harmos-cpp-tests PRIVATE harmos_cpp_fixture)
    foreach(case catalog_rejects_duplicate_operations observer_preserves_its_stream_contract catalog_preserves_typed_sections_and_dependencies)
        add_test(NAME ${case} COMMAND harmos-cpp-tests ${case})
    endforeach()
endif()
