target_include_directories(hfst PRIVATE 
    ${CMAKE_CURRENT_SOURCE_DIR})

bison_target(XreParse xre_parse.yy 
    ${CMAKE_CURRENT_BINARY_DIR}/xre_parse.cc
    DEFINES_FILE ${CMAKE_CURRENT_BINARY_DIR}/xre_parse.hh)
flex_target(XreLex xre_lex.ll
    ${CMAKE_CURRENT_BINARY_DIR}/xre_lex.cc
    DEFINES_FILE ${CMAKE_CURRENT_BINARY_DIR}/xre_lex.hh)
bison_target(PmatchParse pmatch_parse.yy 
    ${CMAKE_CURRENT_BINARY_DIR}/pmatch_parse.cc
    DEFINES_FILE ${CMAKE_CURRENT_BINARY_DIR}/pmatch_parse.hh)
flex_target(PmatchLex pmatch_lex.ll
    ${CMAKE_CURRENT_BINARY_DIR}/pmatch_lex.cc
    DEFINES_FILE ${CMAKE_CURRENT_BINARY_DIR}/pmatch_lex.hh)
flex_target(LexcLex lexc-lexer.ll
    ${CMAKE_CURRENT_BINARY_DIR}/lexc-lexer.cc
    DEFINES_FILE ${CMAKE_CURRENT_BINARY_DIR}/lexc-lexer.hh)
bison_target(LexcParse lexc-parser.yy
    ${CMAKE_CURRENT_BINARY_DIR}/lexc-parser.cc
    DEFINES_FILE ${CMAKE_CURRENT_BINARY_DIR}/lexc-parser.hh)

if(HAVE_SFST)
flex_target(SfstScanner sfst-scanner.ll
    ${CMAKE_CURRENT_BINARY_DIR}/sfst-scanner.cc
    DEFINES_FILE ${CMAKE_CURRENT_BINARY_DIR}/sfst-scanner.hh)
bison_target(SfstCompiler sfst-compiler.yy
    ${CMAKE_CURRENT_BINARY_DIR}/sfst-compiler.cc
    DEFINES_FILE ${CMAKE_CURRENT_BINARY_DIR}/sfst-compiler.hh)
endif()

flex_target(XfstLex xfst-lexer.ll
    ${CMAKE_CURRENT_BINARY_DIR}/xfst-lexer.cc
    DEFINES_FILE ${CMAKE_CURRENT_BINARY_DIR}/xfst-lexer.hh)
bison_target(XfstParse xfst-parser.yy
    ${CMAKE_CURRENT_BINARY_DIR}/xfst-parser.cc
    DEFINES_FILE ${CMAKE_CURRENT_BINARY_DIR}/xfst-parser.hh)

flex_target(Htwolcpre1Lex htwolcpre1-lexer.ll
    ${CMAKE_CURRENT_BINARY_DIR}/htwolcpre1-lexer.cc
    DEFINES_FILE ${CMAKE_CURRENT_BINARY_DIR}/htwolcpre1-lexer.hh)
bison_target(Htwolcpre1Parse htwolcpre1-parser.yy
    ${CMAKE_CURRENT_BINARY_DIR}/htwolcpre1-parser.cc
    DEFINES_FILE ${CMAKE_CURRENT_BINARY_DIR}/htwolcpre1-parser.hh)

flex_target(Htwolcpre2Lex htwolcpre2-lexer.ll
    ${CMAKE_CURRENT_BINARY_DIR}/htwolcpre2-lexer.cc
    DEFINES_FILE ${CMAKE_CURRENT_BINARY_DIR}/htwolcpre2-lexer.hh)
bison_target(Htwolcpre2Parse htwolcpre2-parser.yy
    ${CMAKE_CURRENT_BINARY_DIR}/htwolcpre2-parser.cc
    DEFINES_FILE ${CMAKE_CURRENT_BINARY_DIR}/htwolcpre2-parser.hh)

flex_target(Htwolcpre3Lex htwolcpre3-lexer.ll
    ${CMAKE_CURRENT_BINARY_DIR}/htwolcpre3-lexer.cc
    DEFINES_FILE ${CMAKE_CURRENT_BINARY_DIR}/htwolcpre3-lexer.hh)
bison_target(Htwolcpre3Parse htwolcpre3-parser.yy
    ${CMAKE_CURRENT_BINARY_DIR}/htwolcpre3-parser.cc
    DEFINES_FILE ${CMAKE_CURRENT_BINARY_DIR}/htwolcpre3-parser.hh)

add_flex_bison_dependency(
    XreLex
    XreParse
)

add_library(parsers OBJECT 
    ${BISON_XreParse_OUTPUTS}
    ${FLEX_XreLex_OUTPUTS}
    ${BISON_PmatchParse_OUTPUTS}
    ${FLEX_PmatchLex_OUTPUTS}
    ${FLEX_LexcLex_OUTPUTS}
    ${BISON_LexcParse_OUTPUTS}
    ${FLEX_SfstScanner_OUTPUTS}
    ${BISON_SfstCompiler_OUTPUTS}
    ${FLEX_XfstLex_OUTPUTS}
    ${BISON_XfstParse_OUTPUTS}
    ${FLEX_Htwolcpre1Lex_OUTPUTS}
    ${BISON_Htwolcpre1Parse_OUTPUTS}
    ${FLEX_Htwolcpre2Lex_OUTPUTS}
    ${BISON_Htwolcpre2Parse_OUTPUTS}
    ${FLEX_Htwolcpre3Lex_OUTPUTS}
    ${BISON_Htwolcpre3Parse_OUTPUTS})

target_include_directories(parsers PRIVATE 
    ${ICU_INCLUDE_DIRS}
    ${CMAKE_SOURCE_DIR}
    ${CMAKE_CURRENT_BINARY_DIR}
    "${CMAKE_SOURCE_DIR}/back-ends"
    "${CMAKE_SOURCE_DIR}/back-ends/openfst/src/include"
    "${CMAKE_SOURCE_DIR}/libhfst/src"
)

add_dependencies(hfst parsers)


if(HAVE_SFST)
    target_sources(hfst PUBLIC
        SfstCompiler.cc SfstUtf8.cc SfstBasic.cc SfstAlphabet.cc)
endif()

target_sources(hfst PUBLIC
    xre_utils.cc XreCompiler.cc
    pmatch_utils.cc PmatchCompiler.cc
    lexc-utils.cc LexcCompiler.cc
    xfst-utils.cc XfstCompiler.cc xfst_help_message.cc
   
    TwolcCompiler.cc 
    # commandline_src/CommandLine.cc 
    alphabet_src/Alphabet.cc 
    io_src/InputReader.cc 
    rule_src/ConflictResolvingLeftArrowRule.cc 
    rule_src/ConflictResolvingRightArrowRule.cc 
    rule_src/LeftArrowRule.cc 
    rule_src/LeftArrowRuleContainer.cc 
    rule_src/LeftRestrictionArrowRule.cc 
    rule_src/OtherSymbolTransducer.cc 
    rule_src/RightArrowRule.cc 
    rule_src/RightArrowRuleContainer.cc 
    rule_src/Rule.cc 
    rule_src/RuleContainer.cc 
    rule_src/TwolCGrammar.cc 
    string_src/string_manipulation.cc 
    variable_src/RuleSymbolVector.cc 
    variable_src/RuleVariables.cc 
    variable_src/RuleVariablesConstIterator.cc 
    variable_src/VariableValues.cc
)