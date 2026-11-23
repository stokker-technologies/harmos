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

    add_library(harmos_client_fixture STATIC)
    harmos_protocol(harmos_client_fixture "${root}/sdk/tests/fixtures/client/proto"
        "${CMAKE_CURRENT_BINARY_DIR}/generated" SYNC_SERVER ${fixture_sync_protocols}
        IMPORT_DIRS "${root}/proto")
    harmos_protocol(harmos_client_fixture "${root}/sdk/tests/fixtures/client/proto"
        "${CMAKE_CURRENT_BINARY_DIR}/generated" ${fixture_callback_protocols}
        IMPORT_DIRS "${root}/proto")

    target_link_libraries(harmos_client_fixture PUBLIC Harmos::Cpp)
    if(NOT HARMOS_EXECUTABLE)
        message(FATAL_ERROR "Client Fixture: Configure With -DHARMOS_EXECUTABLE=<harmos CLI>")
    endif()
    set(library_descriptors "${root}/crates/harmos-protocol/src/descriptors.bin")
    set(application_descriptors "${root}/sdk/rust/tests/fixtures/client/client-descriptors.bin")
    set(fixture_generated "${CMAKE_CURRENT_BINARY_DIR}/generated")
    add_custom_command(OUTPUT "${fixture_generated}/client.hpp"
        BYPRODUCTS "${fixture_generated}/detail/client.hpp" "${fixture_generated}/client_messages.hpp"
            "${fixture_generated}/plugin.hpp" "${fixture_generated}/plugin.cpp"
        COMMAND "${HARMOS_EXECUTABLE}" codegen cpp --application --cpp-namespace fixture
            --descriptors "${library_descriptors}" --descriptors "${application_descriptors}" --out "${fixture_generated}"
        DEPENDS "${HARMOS_EXECUTABLE}" "${library_descriptors}" "${application_descriptors}"
        VERBATIM)

    target_include_directories(harmos_client_fixture PUBLIC "${CMAKE_CURRENT_BINARY_DIR}/generated")
    add_custom_target(harmos_product_fixture_roots DEPENDS "${CMAKE_CURRENT_BINARY_DIR}/generated/client.hpp")
    add_dependencies(harmos_client_fixture harmos_product_fixture_roots)
    set_source_files_properties("${CMAKE_CURRENT_BINARY_DIR}/generated/plugin.cpp" PROPERTIES GENERATED TRUE)
    target_sources(harmos_client_fixture PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/generated/plugin.cpp")
endfunction()
