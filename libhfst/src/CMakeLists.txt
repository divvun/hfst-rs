if(NOT WIN32)
    target_compile_options(hfst PRIVATE
        "-Wno-deprecated" "-Wall" "-Wextra"
        "-Wno-unused-parameter" "-Wno-sign-compare" "-g"
        "-I."
        "-I${PROJECT_SOURCE_DIR}/libhfst/src"
        "-I${PROJECT_SOURCE_DIR}/back-ends")
else()
    target_link_libraries(hfst PRIVATE dlfcn-win32::dl)
    target_compile_options(hfst PRIVATE
        "/I."
        "/I${PROJECT_SOURCE_DIR}/libhfst/src"
        "/I${PROJECT_SOURCE_DIR}/back-ends")
endif()

target_include_directories(hfst PRIVATE
    ${ICU_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR})

find_library(UC icuuc)
if(NOT WIN32)
find_library(I18N icui18n)
find_library(DATA icudata)
else()
find_library(I18N icuin)
find_library(DATA icudt)
endif()

add_subdirectory(parsers)
add_subdirectory(implementations)
target_sources(hfst PRIVATE
    HfstApply.cc HfstInputStream.cc HfstTransducer.cc HfstOutputStream.cc
    HfstRules.cc HfstXeroxRules.cc HfstDataTypes.cc
    HfstSymbolDefs.cc HfstTokenizer.cc
    HfstFlagDiacritics.cc HfstExceptionDefs.cc
    HarmonizeUnknownAndIdentitySymbols.cc
    HfstLookupFlagDiacritics.cc
    HfstEpsilonHandler.cc HfstStrings2FstTokenizer.cc
    HfstPrintDot.cc HfstPrintPCKimmo.cc hfst-string-conversions.cc
    string-utils.cc
)
target_link_libraries(hfst PRIVATE ${UC} ${I18N} ${DATA} parsers)
if(HAVE_SFST)
    target_link_libraries(hfst PUBLIC sfst)
endif()
if(HAVE_OPENFST)
    target_link_libraries(hfst PUBLIC openfst)
endif()
if(HAVE_FOMA)
    target_link_libraries(hfst PUBLIC foma)
endif()
target_compile_options(hfst PUBLIC ${ICU_CFLAGS_OTHER})

