function(harmos_client_fixture root)
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
    harmos_protocol(harmos_client_fixture "${root}/sdk/tests/fixtures/client/proto"
        "${CMAKE_CURRENT_BINARY_DIR}/generated" SYNC_SERVER ${fixture_sync_protocols}
        IMPORT_DIRS "${root}/proto")
    harmos_protocol(harmos_client_fixture "${root}/sdk/tests/fixtures/client/proto"
        "${CMAKE_CURRENT_BINARY_DIR}/generated" ${fixture_callback_protocols}
        IMPORT_DIRS "${root}/proto")

    target_link_libraries(harmos_client_fixture PUBLIC Harmos::Cpp)
    find_package(Python3 REQUIRED COMPONENTS Interpreter)
    execute_process(
        COMMAND "${Python3_EXECUTABLE}" -c [=[
from pathlib import Path
import harmos_tools.build.cpp
import harmos_tools.build.resources.cpp
import harmos_tools.build.resources.configuration
import harmos_tools.build.resources.descriptor
import harmos_tools.source.protocol

modules = (
    harmos_tools.build.cpp,
    harmos_tools.build.resources.cpp,
    harmos_tools.build.resources.configuration,
    harmos_tools.build.resources.descriptor,
    harmos_tools.source.protocol,
)
print(";".join(Path(module.__file__).as_posix() for module in modules))
]=]
        OUTPUT_VARIABLE tooling_dependencies
        OUTPUT_STRIP_TRAILING_WHITESPACE
        COMMAND_ERROR_IS_FATAL ANY)
    set(fixture_root "${root}/sdk/tests/fixtures/client/proto")
    set(fixture_dependencies ${fixture_protocols})
    list(TRANSFORM fixture_dependencies PREPEND "${fixture_root}/")
    get_target_property(protobuf_includes protobuf::libprotobuf INTERFACE_INCLUDE_DIRECTORIES)
    set(fixture_imports "--proto_path=${root}/proto")
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
            --source-root "${root}"
            --fixture-client "${CMAKE_CURRENT_BINARY_DIR}/generated/client.hpp"
            --fixture-descriptors "${CMAKE_CURRENT_BINARY_DIR}/generated/fixture-descriptors.bin"
        DEPENDS
            ${tooling_dependencies}
            "${CMAKE_CURRENT_BINARY_DIR}/generated/fixture-descriptors.bin"
            "${root}/crates/harmos-protocol/src/descriptors.bin"
        VERBATIM)

    target_include_directories(harmos_client_fixture PUBLIC "${CMAKE_CURRENT_BINARY_DIR}/generated")
    add_custom_target(harmos_product_fixture_roots DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/generated/client.hpp")
    add_dependencies(harmos_client_fixture harmos_product_fixture_roots)
    set_source_files_properties("${CMAKE_CURRENT_BINARY_DIR}/generated/plugin.cpp" PROPERTIES GENERATED TRUE)
    target_sources(harmos_client_fixture PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/generated/plugin.cpp")
endfunction()
