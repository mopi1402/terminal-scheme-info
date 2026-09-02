#!/bin/zsh
# Runs INSIDE a terminal under test: times one query and records everything
# about it in /tmp/tsi-terminals/<name>.txt for scripts/try-terminals.sh.
zmodload zsh/datetime
name=$1
dir=/tmp/tsi-terminals
mkdir -p $dir
bin=${0:a:h}/../target/release/terminal-scheme-info

t0=$EPOCHREALTIME
out=$($bin query 2>&1)
code=$?
t1=$EPOCHREALTIME
{
    echo "terminal=$name"
    echo "TERM=$TERM TERM_PROGRAM=$TERM_PROGRAM COLORTERM=$COLORTERM"
    echo "exit=$code"
    printf 'ms=%.0f\n' $(( (t1 - t0) * 1000 ))
    echo "$out"
} > $dir/$name.txt
