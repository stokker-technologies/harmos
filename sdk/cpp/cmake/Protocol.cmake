# Match Rust Codegen: Discover Every Protocol Below proto/harmos.
get_filename_component(harmos_proto_root "${CMAKE_CURRENT_LIST_DIR}/../../../proto" ABSOLUTE)
file(GLOB_RECURSE HARMOS_PROTOCOLS CONFIGURE_DEPENDS
    RELATIVE "${harmos_proto_root}" "${harmos_proto_root}/harmos/*.proto")
list(SORT HARMOS_PROTOCOLS)

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
