#!/bin/bash

set -e
PY_VER=3.12

PYTHON="python${PY_VER}"; export VENV="venv-${PY_VER}"
set -eu; cd "$(cd -P -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
if ! [ -f $VENV/bin/python ]; then echo " * Creating ${VENV}" && $PYTHON -m venv $VENV; fi
. $VENV/bin/activate 
pip install -r requirements.txt

$PYTHON script.py $@
