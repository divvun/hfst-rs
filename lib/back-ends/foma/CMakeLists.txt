add_library(foma OBJECT)
if(NOT WIN32)
    target_compile_options(foma PRIVATE
        "-Wno-deprecated"
        "-Wdeprecated-non-prototype"
        "-std=c99"
        "-D_XOPEN_SOURCE=500"
    )
endif()
target_sources(foma PRIVATE
    int_stack.c define.c determinize.c apply.c
    rewrite.c lexcread.c topsort.c flags.c minimize.c reverse.c
    extract.c sigma.c structures.c constructions.c
    coaccessible.c io.c utf8.c spelling.c dynarray.c mem.c stringhash.c
    trie.c lex.lexc.c lex.yy.c regex.c
)