find_package(Catch2 3 CONFIG REQUIRED)
include(Catch)

add_library(harmos_client_fixture STATIC)
harmos_protocol(harmos_client_fixture "${CMAKE_CURRENT_SOURCE_DIR}/../../tests/fixtures/client/proto"
    "${CMAKE_CURRENT_BINARY_DIR}/generated" SYNC_SERVER
    application/v1/jobs.proto application/v1/jobs/progress.proto application/v1/plugins/annotations.proto)
target_link_libraries(harmos_client_fixture PUBLIC Harmos::Cpp)
find_package(Python3 REQUIRED COMPONENTS Interpreter)
set(fixture_root "${CMAKE_CURRENT_SOURCE_DIR}/../../tests/fixtures/client/proto")
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/generated/fixture-descriptors.bin"
    COMMAND protobuf::protoc "--proto_path=${fixture_root}" --include_imports
        "--descriptor_set_out=${CMAKE_CURRENT_BINARY_DIR}/generated/fixture-descriptors.bin"
        application/v1/jobs.proto application/v1/jobs/progress.proto application/v1/plugins/annotations.proto
    DEPENDS "${fixture_root}/application/v1/jobs.proto" "${fixture_root}/application/v1/jobs/progress.proto"
        "${fixture_root}/application/v1/plugins/annotations.proto"
    VERBATIM)
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/generated/client.hpp"
    COMMAND "${Python3_EXECUTABLE}" -m harmos_tools.build.cpp
        --fixture-client "${CMAKE_CURRENT_BINARY_DIR}/generated/client.hpp"
        --fixture-descriptors "${CMAKE_CURRENT_BINARY_DIR}/generated/fixture-descriptors.bin"
    WORKING_DIRECTORY "${PROJECT_SOURCE_DIR}/../.."
    DEPENDS "${PROJECT_SOURCE_DIR}/../../tools/python/src/harmos_tools/build/resource_roots.py"
        "${PROJECT_SOURCE_DIR}/../../tools/python/src/harmos_tools/build/cpp.py"
        "${CMAKE_CURRENT_BINARY_DIR}/generated/fixture-descriptors.bin"
        "${PROJECT_SOURCE_DIR}/../../crates/harmos-protocol/src/descriptors.bin"
    VERBATIM)
add_executable(harmos-cpp-tests unit/catalog.cpp unit/client_root.cpp unit/client_call_context.cpp unit/samples.cpp
    "${CMAKE_CURRENT_BINARY_DIR}/generated/client.hpp")
target_include_directories(harmos-cpp-tests PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/generated")
target_include_directories(harmos-cpp-tests PRIVATE "${PROJECT_SOURCE_DIR}/src")
target_link_libraries(harmos-cpp-tests PRIVATE Harmos::Cpp harmos_client_fixture harmos_conformance_protocol Catch2::Catch2)
catch_discover_tests(harmos-cpp-tests PROPERTIES TIMEOUT 30)
