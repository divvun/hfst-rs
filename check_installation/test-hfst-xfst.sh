#!/bin/sh
#
# Test that the installed version of hfst-xfst works.
#

PREFIX=""
if ! [ "$1" = "" ]; then
    PREFIX=$1"/"
fi

if ! ( which ${PREFIX}hfst-xfst 2> /dev/null > /dev/null ); then
    echo "warning: hfst-xfst not found, assumed switched off and skipping it"
    exit 77
fi

rm -f a2b.script
echo "regex a:b;" > a2b.script
echo "save stack a2b.xfst.hfst" >> a2b.script

for format in sfst openfst-tropical foma;
do
    # Check that the tool is installed right..
    if ! (${PREFIX}hfst-xfst -F a2b.script -f $format 2>1 > /dev/null); then
	exit 1
    fi
    # Check that the tool produces right result..
    if ! (echo "a:b" | ${PREFIX}hfst-strings2fst -f $format | ${PREFIX}hfst-compare -s a2b.xfst.hfst); then
	exit 1
    fi
    rm -f a2b.xfst.hfst
done

rm -f a2b.script
