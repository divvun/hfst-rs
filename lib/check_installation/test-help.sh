#!/bin/sh
TOOLSDIR=$1

EXT=
if (uname | egrep "MINGW|mingw" 2>1 > /dev/null); then
    EXT=".exe";
fi

for file in $TOOLSDIR/hfst-* ; do
    f=`echo $file | sed 's/.*\(hfst-.*\)/\1/'`
    if [ "$f" != "hfst-lexc2fst""$EXT" -a \
        "$f" != "hfst-xfst2fst""$EXT" -a \
	"$f" != "hfst-lexc-compiler""$EXT" -a \
	"$f" != "hfst-info""$EXT" -a \
	"$f" != "hfst-twolc-system""$EXT" -a \
	"$f" != "hfst-duplicate""$EXT" -a \
	"$f" != "hfst-preprocess-for-optimized-lookup-format""$EXT" -a \
	"$f" != "hfst-strip-header""$EXT" -a \
	"$f" != "hfst-train-tagger-system""$EXT" -a \
	"$f" != "hfst-affix-guessify""$EXT" ]; then
        if [ -x "$f" -a ! -d "$f" ] ; then
            if ! "$f" --help > help.out ; then
                rm -f help.out
                echo $f has broken help
                exit 1
            fi
            if ! grep -q '^Usage: hfst-' help.out > /dev/null ; then
                rm -f help.out
                echo $f has malformed help
                exit 1
            fi
        fi
    fi
done
rm -f help.out

