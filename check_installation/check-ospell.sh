#!/bin/sh

#
# Perform tests in ./ospell_tests.
#

PREFIX=

if [ "$1" = "-h" -o "$1" = "--help" ]; then
    echo ""
    echo "  Usage:  "$0" [--prefix PREFIX]"
    echo ""
    exit 0;
elif [ "$1" = "--prefix" ]; then
    PREFIX=$2"/";
fi

TESTDIR=ospell_tests

if [ ! -d "$TESTDIR" ]; then
    echo "ERROR: directory" $TESTDIR "does not exist"
    exit 1;
fi

echo "---------------------- "
echo "Testing hfst-ospell... "
echo "---------------------- "

echo ""
echo "Moving to directory" `pwd`"/"$TESTDIR"..."
echo ""
cd $TESTDIR

if ! (${PREFIX}hfst-ospell --help > /dev/null 2> /dev/null); then
    echo "FAIL: hfst-ospell tests did not pass: hfst-ospell was not found"
    echo ""
    echo "Exiting directory" `pwd`"..."
    echo ""

    cd ..
    exit 1
fi

if ! (ls test.zhfst > /dev/null 2> /dev/null); then
    echo "SKIP: file test.zhfst not found"
    echo ""
    echo "Exiting directory" `pwd`"..."
    echo ""

    cd ..
    exit 1
fi

if ! (cat ospell_input.txt | ${PREFIX}hfst-ospell test.zhfst --suggest --time-cutoff=1.0 > /dev/null 2> /dev/null); then
    echo "FAIL: hfst-ospell tests did not pass"
    echo ""
    echo "Exiting directory" `pwd`"..."
    echo ""

    cd ..
    exit 1
fi

if ! (cat ospell_input.txt | ${PREFIX}hfst-ospell test.zhfst --suggest > /dev/null 2> /dev/null); then
    echo "FAIL: hfst-ospell tests did not pass"
    echo ""
    echo "Exiting directory" `pwd`"..."
    echo ""

    cd ..
    exit 1
fi

if ! (cat ospell_input.txt | ${PREFIX}hfst-ospell test.zhfst > /dev/null 2> /dev/null); then
    echo "FAIL: hfst-ospell tests did not pass"
    echo ""
    echo "Exiting directory" `pwd`"..."
    echo ""

    cd ..
    exit 1
fi


echo "PASS: hfst-ospell tests passed"
echo ""
echo "Exiting directory" `pwd`"..."
echo ""

cd ..



