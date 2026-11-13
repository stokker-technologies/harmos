cmake_minimum_required(VERSION 3.24)
if(CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR)
    project(HarmosConformance LANGUAGES C CXX)
    find_package(Harmos CONFIG REQUIRED)
    include(../../cmake/Protocol.cmake)
endif()

add_library(harmos_conformance_protocol STATIC)
harmos_protocol(harmos_conformance_protocol "${CMAKE_CURRENT_SOURCE_DIR}/../../../../proto"
    "${CMAKE_CURRENT_BINARY_DIR}/generated" harmos/conformance/v1/fixture.proto)
target_link_libraries(harmos_conformance_protocol PUBLIC Harmos::Cpp)
add_executable(harmos-cpp-conformance main.cpp install_itself.cpp conformance.cpp)
target_link_libraries(harmos-cpp-conformance PRIVATE Harmos::Cpp harmos_conformance_protocol)
# Keep Producer Artifacts at the Preset's Build Root on Both Platforms.
set_target_properties(harmos-cpp-conformance PROPERTIES RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}")
