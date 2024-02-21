#!/bin/sh

#
# Perform tests in ./python_tests. Before executing this script,
# run copy-python-tests.sh to create ./python_tests.
#

TESTDIR=python_tests

PYTHON=python3
PYTHONPATH=""

if [ "$1" = "--python" ]; then
    PYTHON=$2
fi

if [ "$1" = "--pythonpath" ]; then
    PYTHONPATH=$2
fi

if [ "$3" = "--python" ]; then
    PYTHON=$4
fi

if [ "$3" = "--pythonpath" ]; then
    PYTHONPATH=$4
fi


if [ ! -d "$TESTDIR" ]; then
    echo "ERROR: directory" $TESTDIR "does not exist, try running ./copy-python-tests.sh first."
    exit 1;
fi

echo "----------------------------------- "
echo "Testing Python bindings for HFST... "
echo "----------------------------------- "

echo ""
echo "Moving to directory" `pwd`"/"$TESTDIR"..."
echo ""
cd $TESTDIR

rm -f tmp.py
touch tmp.py
if ! [ "$PYTHONPATH" = "" ]; then
    echo "import sys" >> tmp.py
    echo "sys.path.insert(1, '"$PYTHONPATH"')" >> tmp.py
fi
cat test_hfst.py >> tmp.py

if ! ( $PYTHON tmp.py > /dev/null 2> /dev/null ); then
    echo "FAIL: test_hfst.py failed"
    echo ""
    echo "Exiting directory" `pwd`"..."
    echo ""
    cd ..
    exit 1
fi

rm -f tmp.py
touch tmp.py
if ! [ "$PYTHONPATH" = "" ]; then
    echo "import sys" >> tmp.py
    echo "sys.path.insert(1, '"$PYTHONPATH"')" >> tmp.py
fi
cat examples.py >> tmp.py

if ! ( $PYTHON tmp.py > /dev/null 2> /dev/null ); then
    echo "FAIL: examples.py failed"
    echo ""
    echo "Exiting directory" `pwd`"..."
    echo ""
    cd ..
    exit 1
fi

echo "PASS: Python tests passed"
echo ""
echo "Exiting directory" `pwd`"..."
echo ""

cd ..



