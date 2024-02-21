#!/bin/sh

#
# Perform tests in ./tool_tests. Before executing this script,
# run copy-tool-tests.sh to create ./tool_tests.
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

TESTDIR=tool_tests

if [ ! -d "$TESTDIR" ]; then
    echo "ERROR: directory" $TESTDIR "does not exist, try running ./copy-tool-tests.sh first."
    exit 1;
fi

echo "---------------------------------- "
echo "Testing HFST command line tools... "
echo "---------------------------------- "

echo ""
echo "Moving to directory" `pwd`"/"$TESTDIR"..."
echo ""
cd $TESTDIR

# Make the transducers needed by the tests.
# Overgenerating some files here..
for txt_file in *.txt;
do
    if ! (echo $txt_file | grep 'pmatch_' > /dev/null 2> /dev/null); then
        echo "generating binary transducers from file "$txt_file"..."
        file=`echo $txt_file | sed 's/\.txt//'`
        ${PREFIX}hfst-txt2fst -e '@0@' $file.txt > $file.hfst
        if (${PREFIX}hfst-format --list-formats | grep 'sfst' > /dev/null); then
	    ${PREFIX}hfst-txt2fst -f sfst -e '@0@' $file.txt > $file.sfst
        else
	    echo "warning: sfst back-end not available, assumed skipped off and continuing"
        fi
        ${PREFIX}hfst-txt2fst -f openfst-tropical -e '@0@' $file.txt > $file.ofst
        if (${PREFIX}hfst-format --list-formats | grep 'foma' > /dev/null); then
	    ${PREFIX}hfst-txt2fst -f foma -e '@0@' $file.txt > $file.foma
        else
	    echo "warning: foma back-end not available, assumed skipped off and continuing"
        fi
        ${PREFIX}hfst-txt2fst -e '@0@' -i $file.txt | ${PREFIX}hfst-fst2fst -w -o $file.hfstol
        ${PREFIX}hfst-txt2fst -e '@0@' -i $file.txt | ${PREFIX}hfst-invert | ${PREFIX}hfst-fst2fst -w -o $file.genhfstol
    else
        echo "generating pmatch transducer from file "$txt_file"..."
        if ! [ -x ${PREFIX}hfst-pmatch2fst ]; then
            echo "tool hfst-pmatch2fst not found, assumed skipped off and continuing"
        else
            file=`echo $txt_file | sed 's/\.txt//'`
            ${PREFIX}hfst-pmatch2fst $file.txt > $file.pmatch
        fi
    fi
done


# Perform the tests
echo ""
echo "performing tests..."
echo ""
for tooltest in *.sh;
do
    if [ "$tooltest" != "copy-files.sh" -a \
	"$tooltest" != "lexc2fst-stress.sh" -a \
	"$tooltest" != "valgrind.sh" -a \
	"$tooltest" != "lookup-stress.sh" ]; then
	if (./$tooltest $PREFIX 2> /dev/null) ; then
	    printf "%-40s%s\n" $tooltest "PASS"
	elif [ "$?" -eq "77" ] ; then
            printf "%-40s%s\n" $tooltest "** SKIP **"
        else
	    printf "%-40s%s\n" $tooltest "*** FAIL ***"
	fi
    fi
done

echo ""
echo "Exiting directory" `pwd`"..."
echo ""

cd ..
