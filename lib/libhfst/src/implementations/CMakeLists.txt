target_include_directories(hfst PRIVATE "${CMAKE_SOURCE_DIR}/back-ends/foma")

if(HAVE_SFST)
    target_sources(hfst PRIVATE
        SfstTransducer.cc
        ConvertSfstTransducer.cc 
    )
endif()

if(HAVE_OPENFST)
    target_sources(hfst PRIVATE
        TropicalWeightTransducer.cc
        LogWeightTransducer.cc
        ConvertTropicalWeightTransducer.cc 
        ConvertLogWeightTransducer.cc 
        optimized-lookup/convert.cc 
    )
endif()

if(HAVE_FOMA)
    target_sources(hfst PRIVATE
        FomaTransducer.cc
        ConvertFomaTransducer.cc 
    )
endif()

target_sources(hfst PRIVATE
    HfstOlTransducer.cc
    ConvertTransducerFormat.cc 
    HfstBasicTransition.cc
    HfstBasicTransducer.cc 
    ConvertOlTransducer.cc
    ConvertXfsmTransducer.cc 
    HfstTropicalTransducerTransitionData.cc 
    compose_intersect/ComposeIntersectRulePair.cc 
    compose_intersect/ComposeIntersectLexicon.cc 
    compose_intersect/ComposeIntersectRule.cc 
    compose_intersect/ComposeIntersectFst.cc 
    compose_intersect/ComposeIntersectUtilities.cc
    optimized-lookup/transducer.cc 
    optimized-lookup/ospell.cc
    optimized-lookup/pmatch.cc 
    optimized-lookup/pmatch_tokenize.cc 
    optimized-lookup/find_epsilon_loops.cc)