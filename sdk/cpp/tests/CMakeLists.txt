find_package(Catch2 3 CONFIG REQUIRED)
include(Catch)

add_library(harmos_client_fixture STATIC)
harmos_protocol(harmos_client_fixture "${CMAKE_CURRENT_SOURCE_DIR}/../../fixtures/client/proto"
    "${CMAKE_CURRENT_BINARY_DIR}/generated" SYNC_SERVER
    library/v1/plugins.proto library/v1/plugins/settings.proto
    application/v1/jobs.proto application/v1/jobs/progress.proto)
target_link_libraries(harmos_client_fixture PUBLIC Harmos::Cpp)
find_package(Python3 REQUIRED COMPONENTS Interpreter)
add_custom_command(OUTPUT "${CMAKE_CURRENT_BINARY_DIR}/generated/client.hpp"
    COMMAND "${Python3_EXECUTABLE}" -m harmos_tools.build.resource_roots
        --cpp-fixture "${CMAKE_CURRENT_BINARY_DIR}/generated/client.hpp"
    WORKING_DIRECTORY "${PROJECT_SOURCE_DIR}/../.."
    DEPENDS "${PROJECT_SOURCE_DIR}/../../tools/python/src/harmos_tools/build/resource_roots.py"
        "${PROJECT_SOURCE_DIR}/../rust/tests/fixtures/client/client-descriptors.bin"
    VERBATIM)
add_executable(harmos-cpp-tests unit/catalog.cpp unit/client_root.cpp unit/client_call_context.cpp
    "${CMAKE_CURRENT_BINARY_DIR}/generated/client.hpp")
target_include_directories(harmos-cpp-tests PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/generated")
target_link_libraries(harmos-cpp-tests PRIVATE Harmos::Cpp harmos_client_fixture harmos_conformance_protocol Catch2::Catch2WithMain)
catch_discover_tests(harmos-cpp-tests)
