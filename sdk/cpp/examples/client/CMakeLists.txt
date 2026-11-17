cmake_minimum_required(VERSION 3.24)
if(CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR)
    project(HarmosClientExample LANGUAGES C CXX)
    find_package(Harmos CONFIG REQUIRED)
    include(../../cmake/Protocol.cmake)
endif()

get_filename_component(repository_root "${CMAKE_CURRENT_SOURCE_DIR}/../../../.." ABSOLUTE)

set(fixture_sync_protocols
    application/host/v1/jobs.proto
    application/host/v1/plugins/annotations.proto
)
set(fixture_callback_protocols
    application/host/v1/access/certificates.proto
    application/host/v1/access/listener.proto
    application/host/v1/access/pairings.proto
    application/host/v1/invocations.proto
    application/host/v1/jobs/progress.proto
    application/host/v1/operations.proto
    application/host/v1/plugins.proto
    application/host/v1/plugins/settings.proto
    application/host/v1/schemas.proto
    application/host/v1/status.proto
    application/host/v1/status/diagnostics.proto
    application/host/v1/status/logs.proto
    application/host/v1/status/trace_events.proto
    application/host/v1/streams.proto
    application/host/v1/time.proto
)
set(fixture_protocols ${fixture_sync_protocols} ${fixture_callback_protocols})

add_library(harmos_client_fixture STATIC)
harmos_protocol(harmos_client_fixture "${CMAKE_CURRENT_SOURCE_DIR}/../../../tests/fixtures/client/proto"
    "${CMAKE_CURRENT_BINARY_DIR}/generated" SYNC_SERVER ${fixture_sync_protocols}
    IMPORT_DIRS "${repository_root}/proto")
harmos_protocol(harmos_client_fixture "${CMAKE_CURRENT_SOURCE_DIR}/../../../tests/fixtures/client/proto"
    "${CMAKE_CURRENT_BINARY_DIR}/generated" ${fixture_callback_protocols}
    IMPORT_DIRS "${repository_root}/proto")

target_link_libraries(harmos_client_fixture PUBLIC Harmos::Cpp)
find_package(Python3 REQUIRED COMPONENTS Interpreter)
execute_process(
    COMMAND "${Python3_EXECUTABLE}" -c "import harmos_tools.build.cpp; print(harmos_tools.build.cpp.ROOT.as_posix())"
    OUTPUT_VARIABLE tooling_root
    OUTPUT_STRIP_TRAILING_WHITESPACE
    COMMAND_ERROR_IS_FATAL ANY)
set(fixture_root "${CMAKE_CURRENT_SOURCE_DIR}/../../../tests/fixtures/client/proto")
set(fixture_dependencies ${fixture_protocols})
list(TRANSFORM fixture_dependencies PREPEND "${fixture_root}/")
get_target_property(protobuf_includes protobuf::libprotobuf INTERFACE_INCLUDE_DIRECTORIES)
set(fixture_imports "--proto_path=${repository_root}/proto")
foreach(directory IN LISTS protobuf_includes)
    list(APPEND fixture_imports "--proto_path=${directory}")
endforeach()
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/generated/fixture-descriptors.bin"
    COMMAND protobuf::protoc "--proto_path=${fixture_root}" ${fixture_imports} --include_imports
        "--descriptor_set_out=${CMAKE_CURRENT_BINARY_DIR}/generated/fixture-descriptors.bin"
        ${fixture_protocols}
    DEPENDS ${fixture_dependencies}
    VERBATIM)
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/generated/client.hpp"
    BYPRODUCTS "${CMAKE_CURRENT_BINARY_DIR}/generated/plugin.hpp" "${CMAKE_CURRENT_BINARY_DIR}/generated/plugin.cpp"
    COMMAND "${Python3_EXECUTABLE}" -m harmos_tools.build.cpp
        --fixture-client "${CMAKE_CURRENT_BINARY_DIR}/generated/client.hpp"
        --fixture-descriptors "${CMAKE_CURRENT_BINARY_DIR}/generated/fixture-descriptors.bin"
    WORKING_DIRECTORY "${tooling_root}"
    DEPENDS
        "${tooling_root}/tools/python/src/harmos_tools/build/resource_roots.py"
        "${tooling_root}/tools/python/src/harmos_tools/build/resources/descriptor.py"
        "${tooling_root}/tools/python/src/harmos_tools/build/resources/cpp.py"
        "${tooling_root}/tools/python/src/harmos_tools/build/cpp.py"
        "${tooling_root}/tools/python/src/harmos_tools/source/protocol.py"
        "${tooling_root}/sdk/cpp/.clang-format"
        "${CMAKE_CURRENT_BINARY_DIR}/generated/fixture-descriptors.bin"
        "${tooling_root}/crates/harmos-protocol/src/descriptors.bin"
    VERBATIM)

target_include_directories(harmos_client_fixture PUBLIC "${CMAKE_CURRENT_BINARY_DIR}/generated")
add_custom_target(harmos_product_fixture_roots DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/generated/client.hpp")
add_dependencies(harmos_client_fixture harmos_product_fixture_roots)
set_source_files_properties("${CMAKE_CURRENT_BINARY_DIR}/generated/plugin.cpp" PROPERTIES GENERATED TRUE)
target_sources(harmos_client_fixture PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/generated/plugin.cpp")

add_executable(harmos-cpp-client main.cpp)
target_link_libraries(harmos-cpp-client PRIVATE Harmos::Cpp harmos_client_fixture)
target_compile_options(harmos-cpp-client PRIVATE ${HARMOS_CXX_WARNINGS})
set_target_properties(harmos-cpp-client PROPERTIES RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}")
