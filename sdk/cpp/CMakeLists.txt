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
set(HARMOS_PROTOCOLS harmos/v1/types harmos/v1/storage_types harmos/conformance/v1/fixture)
file(GLOB_RECURSE HARMOS_PLUGIN_PROTOCOLS CONFIGURE_DEPENDS "${HARMOS_PROTO_ROOT}/harmos/plugin/v1/*.proto" "${HARMOS_PROTO_ROOT}/harmos/context/v1/*.proto")
foreach(protocol IN LISTS HARMOS_PLUGIN_PROTOCOLS)
    file(RELATIVE_PATH relative "${HARMOS_PROTO_ROOT}" "${protocol}")
    string(REGEX REPLACE "\\.proto$" "" relative "${relative}")
    list(APPEND HARMOS_PROTOCOLS "${relative}")
endforeach()
file(GLOB_RECURSE HARMOS_CLIENT_PROTOCOLS CONFIGURE_DEPENDS "${HARMOS_PROTO_ROOT}/harmos/v1/*.proto")
foreach(protocol IN LISTS HARMOS_CLIENT_PROTOCOLS)
    file(RELATIVE_PATH relative "${HARMOS_PROTO_ROOT}" "${protocol}")
    if(relative MATCHES "^harmos/v1/(types|guest|storage_types)\\.proto$")
        continue()
    endif()
    string(REGEX REPLACE "\\.proto$" "" relative "${relative}")
    list(APPEND HARMOS_PROTOCOLS "${relative}")
endforeach()
set(HARMOS_PROTO_INPUTS)
set(HARMOS_PROTO_OUTPUTS)
foreach(protocol IN LISTS HARMOS_PROTOCOLS)
    list(APPEND HARMOS_PROTO_INPUTS "${HARMOS_PROTO_ROOT}/${protocol}.proto")
    list(APPEND HARMOS_PROTO_OUTPUTS "${HARMOS_GENERATED}/${protocol}.pb.cc" "${HARMOS_GENERATED}/${protocol}.pb.h")
    if(NOT protocol MATCHES "^harmos/context/")
        list(APPEND HARMOS_PROTO_OUTPUTS "${HARMOS_GENERATED}/${protocol}.grpc.pb.cc" "${HARMOS_GENERATED}/${protocol}.grpc.pb.h")
    endif()
endforeach()
add_custom_command(OUTPUT ${HARMOS_PROTO_OUTPUTS}
    COMMAND $<TARGET_FILE:protoc>
        --proto_path=${HARMOS_PROTO_ROOT}
        --proto_path=${grpc_SOURCE_DIR}/third_party/protobuf/src
        --cpp_out=${HARMOS_GENERATED}
        --grpc_out=allow_sync_server_api=false,allow_cq_api=false:${HARMOS_GENERATED}
        --plugin=protoc-gen-grpc=$<TARGET_FILE:grpc_cpp_plugin> ${HARMOS_PROTO_INPUTS}
    DEPENDS protoc grpc_cpp_plugin ${HARMOS_PROTO_INPUTS}
    VERBATIM)
add_library(harmos_cpp_protocol STATIC ${HARMOS_PROTO_OUTPUTS})

target_include_directories(harmos_cpp_protocol SYSTEM PUBLIC "${HARMOS_GENERATED}")
target_link_libraries(harmos_cpp_protocol PUBLIC grpc++ libprotobuf)
add_library(harmos_cpp STATIC src/catalog.cpp src/context.cpp src/server.cpp src/conformance.cpp)
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

option(HARMOS_BUILD_TESTS "Build SDK Unit Tests and the Conformance Plugin" OFF)
if(HARMOS_BUILD_TESTS)
    add_executable(harmos-cpp-conformance examples/conformance/main.cpp examples/conformance/install_itself.cpp)
    target_link_libraries(harmos-cpp-conformance PRIVATE Harmos::Cpp)

    set(HARMOS_CLIENT_FIXTURE_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../fixtures/client/proto")
    set(HARMOS_CLIENT_FIXTURE_PROTOCOLS library/v1/plugins library/v1/plugins/settings application/v1/jobs application/v1/jobs/progress)
    set(HARMOS_CLIENT_FIXTURE_INPUTS)
    set(HARMOS_CLIENT_FIXTURE_OUTPUTS)
    foreach(protocol IN LISTS HARMOS_CLIENT_FIXTURE_PROTOCOLS)
        list(APPEND HARMOS_CLIENT_FIXTURE_INPUTS "${HARMOS_CLIENT_FIXTURE_ROOT}/${protocol}.proto")
        list(APPEND HARMOS_CLIENT_FIXTURE_OUTPUTS "${HARMOS_GENERATED}/${protocol}.pb.cc" "${HARMOS_GENERATED}/${protocol}.pb.h" "${HARMOS_GENERATED}/${protocol}.grpc.pb.cc" "${HARMOS_GENERATED}/${protocol}.grpc.pb.h")
    endforeach()
    add_custom_command(OUTPUT ${HARMOS_CLIENT_FIXTURE_OUTPUTS}
        COMMAND $<TARGET_FILE:protoc> --proto_path=${HARMOS_CLIENT_FIXTURE_ROOT}
            --cpp_out=${HARMOS_GENERATED} --grpc_out=${HARMOS_GENERATED}
            --plugin=protoc-gen-grpc=$<TARGET_FILE:grpc_cpp_plugin> ${HARMOS_CLIENT_FIXTURE_INPUTS}
        DEPENDS protoc grpc_cpp_plugin ${HARMOS_CLIENT_FIXTURE_INPUTS}
        VERBATIM)
    add_library(harmos_client_fixture STATIC ${HARMOS_CLIENT_FIXTURE_OUTPUTS})
    target_link_libraries(harmos_client_fixture PUBLIC harmos_cpp_protocol)

    enable_testing()
    add_executable(harmos-cpp-tests tests/unit/catalog.cpp tests/unit/client_root.cpp tests/unit/client_call_context.cpp)
    target_link_libraries(harmos-cpp-tests PRIVATE Harmos::Cpp harmos_client_fixture)
    foreach(case one_call_preserves_metadata_deadline_override_and_cancellation client_roots_compose_services catalog_rejects_duplicate_operations observer_preserves_its_stream_contract catalog_preserves_typed_sections_and_dependencies)
        add_test(NAME ${case} COMMAND harmos-cpp-tests ${case})
    endforeach()
endif()
