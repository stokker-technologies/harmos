cmake_minimum_required(VERSION 3.24)
if(CMAKE_SOURCE_DIR STREQUAL CMAKE_CURRENT_SOURCE_DIR)
    project(HarmosPluginExample LANGUAGES C CXX)
    find_package(Harmos CONFIG REQUIRED)
endif()

add_executable(harmos-cpp-plugin main.cpp)
target_link_libraries(harmos-cpp-plugin PRIVATE Harmos::Cpp)
set_target_properties(harmos-cpp-plugin PROPERTIES RUNTIME_OUTPUT_DIRECTORY "${CMAKE_BINARY_DIR}")
