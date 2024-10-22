cmake_minimum_required(VERSION 3.14)

# Set OMMX C++ SDK version
project(ommx VERSION 0.1.0)

# Use C++17
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Protobuf must be required
find_package(Protobuf REQUIRED)

# Create a static library
file(GLOB SRC "src/ommx/v1/*.pb.cc")
add_library(ommx STATIC ${SRC})

target_include_directories(ommx PUBLIC
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}>/src
    $<INSTALL_INTERFACE:include>
)