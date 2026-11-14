# Explicit SDK Protocol Inventory; Conformance and Client Fixtures Are Test Support.
set(HARMOS_PROTOCOLS
    harmos/client/v1/access/certificates.proto
    harmos/client/v1/access/listener.proto
    harmos/client/v1/access/pairings.proto
    harmos/client/v1/invocations.proto
    harmos/client/v1/operations.proto
    harmos/client/v1/plugins.proto
    harmos/client/v1/plugins/settings.proto
    harmos/client/v1/schemas.proto
    harmos/client/v1/status.proto
    harmos/client/v1/status/diagnostics.proto
    harmos/client/v1/status/logs.proto
    harmos/client/v1/status/trace_events.proto
    harmos/client/v1/streams.proto
    harmos/client/v1/time.proto
    harmos/plugin/host/v1/invocations.proto
    harmos/plugin/host/v1/operations.proto
    harmos/plugin/host/v1/settings.proto
    harmos/plugin/host/v1/time.proto
    harmos/plugin/v1/instance.proto
    harmos/v1/description.proto
    harmos/v1/failure.proto
    harmos/v1/file.proto
    harmos/v1/guest.proto
    harmos/v1/invocation.proto
    harmos/v1/operation.proto
    harmos/v1/page.proto
    harmos/v1/policy.proto
    harmos/v1/position.proto
    harmos/v1/receipt.proto
    harmos/v1/schema.proto
    harmos/v1/setting.proto
    harmos/v1/stream.proto
    harmos/v1/transfer.proto)

function(harmos_protocol target root output)
    cmake_parse_arguments(PROTOCOL "SYNC_SERVER" "" "" ${ARGN})
    set(inputs)
    foreach(protocol IN LISTS PROTOCOL_UNPARSED_ARGUMENTS)
        list(APPEND inputs "${root}/${protocol}")
    endforeach()
    file(MAKE_DIRECTORY "${output}")
    get_target_property(protobuf_includes protobuf::libprotobuf INTERFACE_INCLUDE_DIRECTORIES)
    protobuf_generate(TARGET ${target} PROTOS ${inputs}
        IMPORT_DIRS "${root}" ${protobuf_includes} PROTOC_OUT_DIR "${output}"
        DEPENDENCIES protobuf::protoc ${inputs})
    set(server_options "allow_sync_server_api=false,allow_cq_api=false")
    if(PROTOCOL_SYNC_SERVER)
        set(server_options "allow_sync_server_api=true,allow_cq_api=true")
    endif()
    protobuf_generate(TARGET ${target} LANGUAGE grpc PROTOS ${inputs}
        IMPORT_DIRS "${root}" ${protobuf_includes} PROTOC_OUT_DIR "${output}"
        GENERATE_EXTENSIONS .grpc.pb.h .grpc.pb.cc
        PLUGIN "protoc-gen-grpc=$<TARGET_FILE:gRPC::grpc_cpp_plugin>"
        PLUGIN_OPTIONS "${server_options}"
        DEPENDENCIES protobuf::protoc gRPC::grpc_cpp_plugin ${inputs})
    target_include_directories(${target} SYSTEM PUBLIC
        $<BUILD_INTERFACE:${output}>
        $<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}>)
    target_compile_features(${target} PUBLIC cxx_std_20)
    target_link_libraries(${target} PUBLIC protobuf::libprotobuf gRPC::grpc++)
endfunction()
