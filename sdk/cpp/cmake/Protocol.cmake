# Explicit SDK Protocol Inventory; Conformance and Client Fixtures Are Test Support.
set(HARMOS_PROTOCOLS
    harmos/context/v1/catalog_entries.proto
    harmos/context/v1/host_values.proto
    harmos/context/v1/invocations.proto
    harmos/context/v1/samples.proto
    harmos/context/v1/storage.proto
    harmos/context/v1/time.proto
    harmos/plugin/v1/instance.proto
    harmos/v1/access/certificates.proto
    harmos/v1/access/listener.proto
    harmos/v1/access/pairings.proto
    harmos/v1/catalog_entries.proto
    harmos/v1/invocations.proto
    harmos/v1/plugins.proto
    harmos/v1/status.proto
    harmos/v1/status/diagnostics.proto
    harmos/v1/status/logs.proto
    harmos/v1/status/trace_events.proto
    harmos/v1/storage.proto
    harmos/v1/storage_settings.proto
    harmos/v1/storage_types.proto
    harmos/v1/time.proto
    harmos/v1/types.proto)

function(harmos_protocol target root output)
    cmake_parse_arguments(PROTOCOL "SYNC_SERVER" "" "" ${ARGN})
    set(inputs)
    foreach(protocol IN LISTS PROTOCOL_UNPARSED_ARGUMENTS)
        list(APPEND inputs "${root}/${protocol}")
    endforeach()
    file(MAKE_DIRECTORY "${output}")
    get_target_property(protobuf_includes protobuf::libprotobuf INTERFACE_INCLUDE_DIRECTORIES)
    protobuf_generate(TARGET ${target} PROTOS ${inputs}
        IMPORT_DIRS "${root}" ${protobuf_includes} PROTOC_OUT_DIR "${output}")
    set(server_options "allow_sync_server_api=false,allow_cq_api=false")
    if(PROTOCOL_SYNC_SERVER)
        set(server_options "allow_sync_server_api=true,allow_cq_api=true")
    endif()
    protobuf_generate(TARGET ${target} LANGUAGE grpc PROTOS ${inputs}
        IMPORT_DIRS "${root}" ${protobuf_includes} PROTOC_OUT_DIR "${output}"
        GENERATE_EXTENSIONS .grpc.pb.h .grpc.pb.cc
        PLUGIN "protoc-gen-grpc=$<TARGET_FILE:gRPC::grpc_cpp_plugin>"
        PLUGIN_OPTIONS "${server_options}"
        DEPENDENCIES gRPC::grpc_cpp_plugin)
    target_include_directories(${target} SYSTEM PUBLIC
        $<BUILD_INTERFACE:${output}>
        $<INSTALL_INTERFACE:include>)
    target_compile_features(${target} PUBLIC cxx_std_20)
    target_link_libraries(${target} PUBLIC protobuf::libprotobuf gRPC::grpc++)
endfunction()
