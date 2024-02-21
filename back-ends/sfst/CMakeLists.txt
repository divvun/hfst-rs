add_library(sfst OBJECT)
target_sources(sfst PRIVATE
    alphabet.cc basic.cc utf8.cc compact.cc fst.cc
    operators.cc determinise.cc generate.cc hopcroft.cc
)