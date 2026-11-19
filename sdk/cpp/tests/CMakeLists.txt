find_package(Catch2 3 CONFIG REQUIRED)
include(Catch)

add_executable(harmos-cpp-tests unit/catalog.cpp unit/client_root.cpp unit/client_call_context.cpp unit/columns.cpp unit/samples.cpp unit/listener.cpp
)
target_include_directories(harmos-cpp-tests PRIVATE "${CMAKE_CURRENT_BINARY_DIR}/generated")
target_include_directories(harmos-cpp-tests PRIVATE "${PROJECT_SOURCE_DIR}/src" "${PROJECT_SOURCE_DIR}/examples/conformance")
target_link_libraries(harmos-cpp-tests PRIVATE Harmos::Cpp harmos_client_fixture harmos_conformance_protocol Catch2::Catch2)
target_compile_options(harmos-cpp-tests PRIVATE ${HARMOS_CXX_WARNINGS})
catch_discover_tests(harmos-cpp-tests PROPERTIES TIMEOUT 30)
