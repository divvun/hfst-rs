#!/bin/sh

#
# Check all installed HFST morphologies.
#

script_prefix=
hfst_tool_prefix=

if [ "$1" = "-h" -o "$1" = "--help" ]; then
    echo ""
    echo "USAGE: "$0" [--script-prefix PRE1] [--hfst-tool-prefix PRE2]"
    echo ""
    echo "PRE1 indicates where the morphology scripts are found."
    echo "PRE2 indicates where the hfst tools used by the morphology scripts are found."
    echo ""
    exit 0
fi

if [ "$1" = "--script-prefix" ]; then
    script_prefix=$2
fi
if [ "$1" = "--hfst-tool-prefix" ]; then
    hfst_tool_prefix=$2
fi

if [ "$3" = "--script-prefix" ]; then
    script_prefix=$4
fi
if [ "$3" = "--hfst-tool-prefix" ]; then
    hfst_tool_prefix=$4
fi

languages="english french german italian finnish swedish turkish"
directions="analyze generate"
format=xerox
morph_folder=morphology_tests


echo "---------------------------- "
echo "Testing HFST morphologies... "
echo "---------------------------- "

for lang in $languages;
do
    for dir in $directions;
    do
	prog=$lang-$dir"-words"
        prog_full_path=

	# test that the program exists
        if [ "$script_prefix" = "" ]; then
	    if (! which $prog > /dev/null 2> /dev/null); then
                printf "%-32s%s\n" $prog "FAIL: program not found"
                exit 1
	    fi
            prog_full_path=`which $prog`
        else
            prog_full_path=$script_prefix/$prog
            if ! [ -x $prog_full_path ]; then
                printf "%-32s%s\n" $prog "FAIL: program not found or executable"
                exit 1
            fi
        fi

        if ! [ "$hfst_tool_prefix" = "" ]; then
            if (! ls $hfst_tool_prefix/hfst-tokenize > /dev/null 2> /dev/null ); then
                echo "FAIL: hfst-tokenize not found in "$hfst_tool_prefix" (given with --hfst-tool-prefix)"
                exit 1
            fi
	    if (! ls $hfst_tool_prefix/hfst-lookup > /dev/null 2> /dev/null ); then
                echo "FAIL: hfst-lookup not found in "$hfst_tool_prefix" (given with --hfst-tool-prefix)"
                exit 1
            fi
            echo "Copying script "$prog_full_path" and modifying it so that is uses hfst tools located in "$hfst_tool_prefix" (given with --hfst-tool-prefix)..."
            hfst_tool_prefix_escaped=${hfst_tool_prefix//\//\\\/}
            echo $hfst_tool_prefix_escaped
            cat $prog_full_path | perl -pe 's/hfst-tokenize /'$hfst_tool_prefix_escaped'\/hfst-tokenize /g;' | perl -pe 's/hfst-lookup /'$hfst_tool_prefix_escaped'\/hfst-lookup /g;' > copied_morphology_script.sh
            chmod u+x copied_morphology_script.sh
            prog_full_path="./copied_morphology_script.sh"
        fi

	# test that the program handles a non-word
	if (! echo "foo" | $prog_full_path $format > /dev/null 2> /dev/null); then
	    printf "%-32s%s\n" $prog "FAIL: program cannot handle input 'foo' (given via standard input)"
            exit 1
	fi

	# test that the program handles real words
	if ! [ "$dir" = "generate" ]; then
	    if (! cat $morph_folder/$lang-$dir-words.input | $prog_full_path $format > $morph_folder/$lang-$dir-words.result); then
		printf "%-32s%s\n" $prog "FAIL: program cannot handle valid input (given via standard input)"
		exit 1
	    fi
	else
	    # use output from analyzer as input to generator
	    if (! cut -f 2- $morph_folder/$lang-analyze-words.result | perl -pe 's/^\n//g;' | $prog_full_path $format > $morph_folder/$lang-$dir-words.result); then
		printf "%-32s%s\n" $prog "FAIL: program cannot handle valid input (given via standard input)"
		exit 1
	    fi
	fi
	# check that the result is as expected
	if ! [ "$dir" = "generate" ]; then
	    if (! diff $morph_folder/$lang-$dir-words.result $morph_folder/$lang-$dir-words.output); then
		printf "%-32s%s\n" $prog "FAIL: wrong result for input (given via standard input)"
		exit 1
	    fi
	else
	    # just check that the generated forms contain each original form given to the analyzer
	    for word in `cat $morph_folder/$lang-analyze-words.input | tr -d '.' | tr -d ','`;
            do
		if ! grep --ignore-case $word $morph_folder/$lang-$dir-words.result > /dev/null 2> /dev/null ; then
		    printf "%-32s%s\n" $prog "FAIL: correct generated form '"$word"' not found"
		    exit 1
		fi
	    done
	fi

	printf "%-32s%s\n" $prog "PASS"
    done
done

rm -f copied_morphology_script.sh
rm -f $morph_folder/*.result

echo "-----------------"
echo "All tests passed."
echo "-----------------"
