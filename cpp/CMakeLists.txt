cmake_minimum_required(VERSION 3.14)

# Set OMMX C++ SDK version
project(ommx VERSION 0.1.0)

# Use C++17
set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Protobuf must be required
find_package(Protobuf REQUIRED)

# Create a static library
file(GLOB SRC "ommx/v1/*.pb.cc")
add_library(ommx STATIC ${SRC})

target_include_directories(ommx PUBLIC 
    $<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}>
    $<INSTALL_INTERFACE:include>
)

target_link_libraries(ommx PUBLIC protobuf::libprotobuf)

install(TARGETS ommx
    EXPORT ommxTargets
    LIBRARY DESTINATION lib
    ARCHIVE DESTINATION lib
    RUNTIME DESTINATION bin
    INCLUDES DESTINATION include
)

install(DIRECTORY ommx
    DESTINATION include
    FILES_MATCHING PATTERN "*.h"
)

install(EXPORT ommxTargets
    FILE ommxTargets.cmake
    NAMESPACE ommx::
    DESTINATION lib/cmake/ommx
)

include(CMakePackageConfigHelpers)
write_basic_package_version_file(
    "ommxConfigVersion.cmake"
    VERSION ${PROJECT_VERSION}
    COMPATIBILITY SameMajorVersion
)

configure_package_config_file(
    "${CMAKE_CURRENT_SOURCE_DIR}/cmake/ommxConfig.cmake.in"
    "${CMAKE_CURRENT_BINARY_DIR}/ommxConfig.cmake"
    INSTALL_DESTINATION lib/cmake/ommx
)

install(FILES
    "${CMAKE_CURRENT_BINARY_DIR}/ommxConfig.cmake"
    "${CMAKE_CURRENT_BINARY_DIR}/ommxConfigVersion.cmake"
    DESTINATION lib/cmake/ommx
)