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


extension=
languages="english french german italian finnish swedish turkish"
directions="analyze generate"
format=xerox
morph_folder=morphology_tests

function exit_prog {
    rm -f input tmp
    exit 1
}

echo "---------------------------- "
echo "Testing HFST morphologies... "
echo "---------------------------- "

for lang in $languages;
do
    for dir in $directions;
    do
	prog=$lang-$dir$extension
        prog_full_path=

	# test that the program exists
        if [ "$script_prefix" = "" ]; then
	    if (! which $prog 2>1 > /dev/null); then
                printf "%-32s%s\n" $prog "FAIL: program not found"
                exit_prog
	    fi
            prog_full_path=`which $prog`
        else
            prog_full_path=$script_prefix/$prog
            if ! [ -x $prog_full_path ]; then
                printf "%-32s%s\n" $prog "FAIL: program not found or executable"
                exit_prog
            fi
        fi

        if ! [ "$hfst_tool_prefix" = "" ]; then
            if (! ls $hfst_tool_prefix/hfst-proc > /dev/null 2> /dev/null ); then
                echo "FAIL: hfst-proc not found in "$hfst_tool_prefix" (given with --hfst-tool-prefix)"
                exit_prog
            fi
            echo "Copying script "$prog_full_path" and modifying it so that is uses hfst tools located in "$hfst_tool_prefix" (given with --hfst-tool-prefix)..."
            hfst_tool_prefix_escaped=${hfst_tool_prefix//\//\\\/}
            echo $hfst_tool_prefix_escaped
            cat $prog_full_path | perl -pe 's/hfst-proc /'$hfst_tool_prefix_escaped'\/hfst-proc /g;' > copied_morphology_script.sh
            chmod u+x copied_morphology_script.sh
            prog_full_path="./copied_morphology_script.sh"
        fi

	# test that the program handles a non-word
        rm -f input
        echo "foo" > input
	# finnish tool accepts input only from standard input
	if ! [ "$lang" = "finnish" ]; then
            if (! $prog_full_path $format input 2>1 > /dev/null); then
		printf "%-32s%s\n" $prog "FAIL: program cannot handle input 'foo' (given as first argument)"
		exit_prog
            fi
	fi

	if (! cat input | $prog_full_path $format 2>1 > /dev/null); then
	    printf "%-32s%s\n" $prog "FAIL: program cannot handle input 'foo' (given via standard input)"
            exit_prog
	fi

	# test that the program handles a real word
	# finnish tool accepts input only from standard input
	if ! [ "$lang" = "finnish" ]; then
	    if (! $prog_full_path $format $morph_folder/$lang-$dir.input > tmp); then
		printf "%-32s%s\n" $prog "FAIL: program cannot handle valid input (given as first argument)"
		exit_prog
	    fi
	    if (! diff tmp $morph_folder/$lang-$dir.output); then
		printf "%-32s%s\n" $prog "FAIL: wrong result for input (given as first argument)"
		exit_prog
	    fi
	fi

	if (! cat $morph_folder/$lang-$dir.input | $prog_full_path $format > tmp); then
	    printf "%-32s%s\n" $prog "FAIL: program cannot handle valid input (given via standard input)"
            exit_prog
	fi
	if (! diff tmp $morph_folder/$lang-$dir.output); then
	    printf "%-32s%s\n" $prog "FAIL: wrong result for input (given via standard input)"
            exit_prog
	fi

	printf "%-32s%s\n" $prog "PASS"
    done
done

rm -f input tmp copied_morphology_script.sh

echo "-----------------"
echo "All tests passed."
echo "-----------------"
