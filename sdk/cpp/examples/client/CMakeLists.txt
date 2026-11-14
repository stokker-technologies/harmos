cmake_minimum_required(VERSION 3.24)
if(CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR)
    project(HarmosClientExample LANGUAGES C CXX)
    find_package(Harmos CONFIG REQUIRED)
endif()

add_executable(harmos-cpp-client main.cpp)
target_link_libraries(harmos-cpp-client PRIVATE Harmos::Cpp)
set_target_properties(harmos-cpp-client PROPERTIES RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}")
