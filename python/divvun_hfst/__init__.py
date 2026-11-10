"""Divvun HFST for Python.

Reads optimized-lookup transducers, pmatch tokenisers and DHFST spellers, and
looks words up, tokenises text and checks words against DHFST acceptors with
them, giving the results the hfst tools give. It reads and looks up; it does
not compile, write or change transducers.
"""

# [spec:hfst:req:python.module]
# The package is imported as divvun_hfst, and its version is the one
# crates/hfst-python carries, apart from the library's.
from divvun_hfst._divvun_hfst import (
    AcceptorInfo,
    DhfstAcceptor,
    DhfstErrorModel,
    FormatError,
    HfstError,
    HfstErrorCode,
    LookupResult,
    OptimisedTransducer,
    Reading,
    Token,
    TokeniseFormat,
    Tokeniser,
    Transducer,
    TransducerType,
    __version__,
    load_dhfst,
    load_optimised_transducer,
    load_tokeniser,
    load_transducer,
    load_transducers,
    read_dhfst,
    read_optimised_transducer,
    read_tokeniser,
    read_transducer,
    read_transducers,
)
from divvun_hfst._options import FormatOptions, TokeniseOptions

__all__ = [
    "AcceptorInfo",
    "DhfstAcceptor",
    "DhfstErrorModel",
    "FormatError",
    "FormatOptions",
    "HfstError",
    "HfstErrorCode",
    "LookupResult",
    "OptimisedTransducer",
    "Reading",
    "Token",
    "TokeniseFormat",
    "TokeniseOptions",
    "Tokeniser",
    "Transducer",
    "TransducerType",
    "__version__",
    "load_dhfst",
    "load_optimised_transducer",
    "load_tokeniser",
    "load_transducer",
    "load_transducers",
    "read_dhfst",
    "read_optimised_transducer",
    "read_tokeniser",
    "read_transducer",
    "read_transducers",
]
