# Explicit SDK Protocol Inventory; Conformance and Client Fixtures Are Test Support.
set(HARMOS_PROTOCOLS
    harmos/host/v1/access/certificates.proto
    harmos/host/v1/access/listener.proto
    harmos/host/v1/access/pairings.proto
    harmos/host/v1/invocations.proto
    harmos/host/v1/operations.proto
    harmos/host/v1/plugins.proto
    harmos/host/v1/plugins/settings.proto
    harmos/host/v1/schema.proto
    harmos/host/v1/schemas.proto
    harmos/host/v1/status.proto
    harmos/host/v1/status/diagnostics.proto
    harmos/host/v1/status/logs.proto
    harmos/host/v1/status/trace_events.proto
    harmos/host/v1/streams.proto
    harmos/host/v1/time.proto
    harmos/model/v1/callers.proto
    harmos/model/v1/description.proto
    harmos/model/v1/failure.proto
    harmos/model/v1/file.proto
    harmos/model/v1/invocation.proto
    harmos/model/v1/operation.proto
    harmos/model/v1/page.proto
    harmos/model/v1/policy.proto
    harmos/model/v1/position.proto
    harmos/model/v1/receipt.proto
    harmos/model/v1/setting.proto
    harmos/model/v1/stream.proto
    harmos/model/v1/transfer.proto
    harmos/plugin/v1/guest.proto
    harmos/plugin/v1/instance.proto
    harmos/plugin/v1/invocation.proto
    harmos/plugin/v1/operation.proto
    harmos/plugin/v1/stream.proto)

function(harmos_protocol target root output)
    cmake_parse_arguments(PROTOCOL "SYNC_SERVER" "" "IMPORT_DIRS" ${ARGN})
    set(inputs)
    foreach(protocol IN LISTS PROTOCOL_UNPARSED_ARGUMENTS)
        list(APPEND inputs "${root}/${protocol}")
    endforeach()
    file(MAKE_DIRECTORY "${output}")
    get_target_property(protobuf_includes protobuf::libprotobuf INTERFACE_INCLUDE_DIRECTORIES)
    protobuf_generate(TARGET ${target} PROTOS ${inputs}
        IMPORT_DIRS "${root}" ${PROTOCOL_IMPORT_DIRS} ${protobuf_includes} PROTOC_OUT_DIR "${output}"
        DEPENDENCIES protobuf::protoc ${inputs})
    set(server_options "allow_sync_server_api=false,allow_cq_api=false")
    if(PROTOCOL_SYNC_SERVER)
        set(server_options "allow_sync_server_api=true,allow_cq_api=true")
    endif()
    protobuf_generate(TARGET ${target} LANGUAGE grpc PROTOS ${inputs}
        IMPORT_DIRS "${root}" ${PROTOCOL_IMPORT_DIRS} ${protobuf_includes} PROTOC_OUT_DIR "${output}"
        GENERATE_EXTENSIONS .grpc.pb.h .grpc.pb.cc
        PLUGIN "protoc-gen-grpc=$<TARGET_FILE:gRPC::grpc_cpp_plugin>"
        PLUGIN_OPTIONS "${server_options}"
        DEPENDENCIES protobuf::protoc gRPC::grpc_cpp_plugin ${inputs})
    target_include_directories(${target} SYSTEM PUBLIC
        $<BUILD_INTERFACE:${output}>
        $<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}/harmos/detail/protocol>)
    target_compile_features(${target} PUBLIC cxx_std_20)
    target_link_libraries(${target} PUBLIC protobuf::libprotobuf gRPC::grpc++)
endfunction()

function(harmos_conformance_protocol target root output)
    harmos_protocol(${target} "${root}" "${output}" conformance/model/v1/fixture.proto)
endfunction()
