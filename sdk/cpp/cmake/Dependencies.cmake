# gRPC 1.76.0 Includes Pinned Protobuf, Abseil, BoringSSL and c-ares Submodules.
include(FetchContent)
set(gRPC_BUILD_TESTS OFF CACHE BOOL "Build Only Runtime and Code Generation" FORCE)
set(gRPC_BUILD_MSVC_MP_COUNT 4 CACHE STRING "Bound gRPC Translation-Unit Parallelism" FORCE)
set(gRPC_INSTALL OFF CACHE BOOL "Dependencies Stay in the Build Tree" FORCE)
set(gRPC_BUILD_GRPC_CSHARP_PLUGIN OFF CACHE BOOL "Generate C++ Only" FORCE)
set(gRPC_BUILD_GRPC_NODE_PLUGIN OFF CACHE BOOL "Generate C++ Only" FORCE)
set(gRPC_BUILD_GRPC_OBJECTIVE_C_PLUGIN OFF CACHE BOOL "Generate C++ Only" FORCE)
set(gRPC_BUILD_GRPC_PHP_PLUGIN OFF CACHE BOOL "Generate C++ Only" FORCE)
set(gRPC_BUILD_GRPC_PYTHON_PLUGIN OFF CACHE BOOL "Generate C++ Only" FORCE)
set(gRPC_BUILD_GRPC_RUBY_PLUGIN OFF CACHE BOOL "Generate C++ Only" FORCE)
set(protobuf_BUILD_TESTS OFF CACHE BOOL "Build Only Runtime and Code Generation" FORCE)
set(protobuf_INSTALL OFF CACHE BOOL "Dependencies Stay in the Build Tree" FORCE)
set(utf8_range_ENABLE_INSTALL OFF CACHE BOOL "Dependencies Stay in the Build Tree" FORCE)
FetchContent_Declare(grpc
    GIT_REPOSITORY https://github.com/grpc/grpc.git
    GIT_TAG f5ffb68d8a2fd603dff16287e90a4ac571e1fec6
    GIT_SUBMODULES third_party/abseil-cpp third_party/boringssl-with-bazel
        third_party/cares/cares third_party/protobuf third_party/re2 third_party/zlib
    GIT_SUBMODULES_RECURSE FALSE)
FetchContent_MakeAvailable(grpc)
