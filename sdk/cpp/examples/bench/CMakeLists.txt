cmake_minimum_required(VERSION 3.24)
project(HarmosBench LANGUAGES CXX)

find_package(Harmos CONFIG REQUIRED)
add_executable(harmos-bench main.cpp)
target_link_libraries(harmos-bench PRIVATE Harmos::Cpp)
