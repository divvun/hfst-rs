add_library(dlfcn OBJECT)
target_sources(dlfcn PUBLIC
    ./dlfcn.c)
# message(${INCLUDES})

# target_include_directories(hfst PUBLIC
#     ${ICU_INCLUDE_DIRS}
#     ${CMAKE_CURRENT_SOURCE_DIR})
# find_library(UC icuuc)
# find_library(I18N icui18n)
# find_library(DATA icudata)
# target_link_libraries(hfst ${UC} ${I18N} ${DATA})
# target_compile_options(hfst PUBLIC ${ICU_CFLAGS_OTHER})