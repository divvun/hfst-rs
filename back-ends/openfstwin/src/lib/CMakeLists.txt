target_include_directories(openfst PUBLIC
    ${ICU_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/../include)
target_sources(openfst PRIVATE
    compat.cc flags.cc fst.cc properties.cc 
	symbol-table.cc symbol-table-ops.cc util.cc
)