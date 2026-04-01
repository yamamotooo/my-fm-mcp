cmake_minimum_required(VERSION 3.16.0)

if(CMAKE_MINOR_VERSION STREQUAL "22")
    set(CMAKE_C_COMPILER "/usr/bin/clang-14")
    set(CMAKE_CXX_COMPILER "/usr/bin/clang++-14")
elseif(CMAKE_MINOR_VERSION STREQUAL "28")
    set(CMAKE_C_COMPILER "/usr/bin/clang-16")
    set(CMAKE_CXX_COMPILER "/usr/bin/clang++-16")
else()
    set(CMAKE_C_COMPILER "/usr/bin/clang-12")
    set(CMAKE_CXX_COMPILER "/usr/bin/clang++-12")
endif()

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

project(FMMiniPlugIn VERSION 1.0.0)

# Compiler options - functional
add_compile_options(-m64)
add_compile_options(-fasm-blocks)
add_compile_options(-fdiagnostics-show-note-include-stack)
add_compile_options(-fexceptions)
add_compile_options(-fmacro-backtrace-limit=0)
add_compile_options(-fmessage-length=0)
add_compile_options(-fPIC)
add_compile_options(-fstrict-aliasing)
add_compile_options(-fstack-protector-strong)

# Compiler options - warnings to enable
add_compile_options(-Wall)
add_compile_options(-Werror)
add_compile_options(-Wnon-virtual-dtor)
add_compile_options(-Wunreachable-code)
add_compile_options(-Wmissing-include-dirs)
add_compile_options(-Wdeprecated-declarations)
add_compile_options(-Winvalid-offsetof)
add_compile_options(-Wmissing-braces)
add_compile_options(-Wmissing-field-initializers)
add_compile_options(-Wparentheses)
add_compile_options(-Wswitch)
add_compile_options(-Wunused-value)
add_compile_options(-Wunused-variable)

# Compiler options - warnings to disable
add_compile_options(-Wno-unused-const-variable)
add_compile_options(-Wno-bool-conversion)
add_compile_options(-Wno-c++11-extensions)
add_compile_options(-Wno-constant-conversion)
add_compile_options(-Wno-conversion)
add_compile_options(-Wno-conversion-null)
add_compile_options(-Wno-deprecated)
add_compile_options(-Wno-empty-body)
add_compile_options(-Wno-enum-conversion)
add_compile_options(-Wno-exit-time-destructors)
add_compile_options(-Wno-four-char-constants)
add_compile_options(-Wno-int-conversion)
add_compile_options(-Wno-missing-prototypes)
add_compile_options(-Wno-newline-eof)
add_compile_options(-Wno-non-virtual-dtor)
add_compile_options(-Wno-overloaded-virtual)
add_compile_options(-Wno-shadow)
add_compile_options(-Wno-shorten-64-to-32)
add_compile_options(-Wno-sign-conversion)
add_compile_options(-Wno-trigraphs)
add_compile_options(-Wno-uninitialized)
add_compile_options(-Wno-unknown-pragmas)
add_compile_options(-Wno-unused-function)
add_compile_options(-Wno-unused-label)
add_compile_options(-Wno-unused-parameter)

if(CMAKE_BUILD_TYPE STREQUAL "Release")
    add_compile_options(-O3)
elseif(CMAKE_BUILD_TYPE STREQUAL "Debug")
    add_compile_options(-g)
endif()

# include header folder
include_directories(
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${CMAKE_CURRENT_SOURCE_DIR}/../Headers
)

# check OS version and cpu platform
execute_process(
    COMMAND lsb_release -r
    OUTPUT_VARIABLE SYSTEM_VERSION
)
string(REGEX MATCH "Release:[\t ]+([0-9]+)\\.[0-9]+" UBUNTU_VERSION "${SYSTEM_VERSION}")
if(CMAKE_MATCH_1 STREQUAL "20")
    set(PLATFORM U20)
elseif(CMAKE_MATCH_1 STREQUAL "22")
    if(CMAKE_HOST_SYSTEM_PROCESSOR MATCHES "aarch64")
        set(PLATFORM U22/arm64)
    elseif(CMAKE_HOST_SYSTEM_PROCESSOR MATCHES "x86_64")
        set(PLATFORM U22/x64)
    endif()
elseif(CMAKE_MATCH_1 STREQUAL "24")
    if(CMAKE_HOST_SYSTEM_PROCESSOR MATCHES "aarch64")
        set(PLATFORM U24/arm64)
    elseif(CMAKE_HOST_SYSTEM_PROCESSOR MATCHES "x86_64")
        set(PLATFORM U24/x64)
    endif()
else()
    message("Not support!")
endif()

# link library foulder
link_directories(
    ${CMAKE_CURRENT_SOURCE_DIR}/../Libraries/Linux/${PLATFORM}
)

# add source file
set(FMMiniPlugin_SRC
    FMMiniPlugIn/FMMiniPlugIn.cpp
)

# specify name and type
add_library(FMMiniPlugin MODULE ${FMMiniPlugin_SRC})

# link needed library
target_link_libraries(FMMiniPlugin PRIVATE
    FMWrapper
)

set_target_properties(FMMiniPlugin PROPERTIES PREFIX "")
set_target_properties(FMMiniPlugin PROPERTIES SUFFIX ".fmx")
set_target_properties(FMMiniPlugin PROPERTIES BUILD_WITH_INSTALL_RPATH TRUE)
set_target_properties(FMMiniPlugin PROPERTIES INSTALL_RPATH "/opt/FileMaker/lib")
set_target_properties(FMMiniPlugin PROPERTIES LIBRARY_OUTPUT_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR}/${CMAKE_BUILD_TYPE})
