# libhfst/src/parsers/grammar_defs.h

> [spec:hfst:def:grammar-defs.symbol]
> typedef std::string Symbol

> [spec:hfst:def:grammar-defs.symbol-pair]
> typedef std::pair<std::string,std::string> SymbolPair

> [spec:hfst:def:grammar-defs.symbol-pair-range]
> typedef std::vector<SymbolPair> SymbolPairRange

> [spec:hfst:def:grammar-defs.symbol-pair-vector]
> typedef std::vector<SymbolPair> SymbolPairVector

> [spec:hfst:def:grammar-defs.symbol-range]
> typedef std::vector<std::string> SymbolRange

> [spec:hfst:def:grammar-defs.symbol-set]
> typedef std::pair<std::string,SymbolRange> SymbolSet

> [spec:hfst:def:grammar-defs.symbol-set-map]
> typedef std::map<std::string,SymbolSet> SymbolSetMap

