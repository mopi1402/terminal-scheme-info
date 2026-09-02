#!/bin/sh
# Runs INSIDE a terminal under test (Linux): times one query and records it in
# $TSI_RESULTS/<name>.txt for scripts/try-terminals-linux.sh.
name=$1
bin=$(dirname "$0")/../target/release/terminal-scheme-info
t0=$(date +%s%N)
out=$("$bin" query 2>&1)
code=$?
t1=$(date +%s%N)
{
    echo "terminal=$name"
    echo "TERM=$TERM COLORTERM=$COLORTERM"
    echo "exit=$code"
    echo "ms=$(( (t1 - t0) / 1000000 ))"
    echo "$out"
} > "${TSI_RESULTS:-/tmp/tsi-terminals}/$name.txt"
