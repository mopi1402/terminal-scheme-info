#!/bin/sh
# Runs one real query in every installed Linux terminal emulator and prints a
# markdown table (terminal, version, OS, answer, time). Needs an X display and
# a session bus; in CI:
#
#   xvfb-run -a -s "-screen 0 1280x800x24" dbus-run-session -- scripts/try-terminals-linux.sh
set -u
cd "$(dirname "$0")/.." || exit 1
results=${TSI_RESULTS:-/tmp/tsi-terminals}
export TSI_RESULTS=$results
rm -rf "$results"
mkdir -p "$results"
probe="$PWD/scripts/probe-terminal.sh"
. /etc/os-release

echo "| Terminal | Version | OS | Answers OSC 10/11 | Time |"
echo "| --- | --- | --- | --- | --- |"

# try SLUG "Display name" PACKAGE COMMAND...   (the command runs the probe)
try() {
    slug=$1 label=$2 pkg=$3
    shift 3
    version=$(dpkg-query -W -f='${Version}' "$pkg" 2>/dev/null)
    if [ -z "$version" ] || ! command -v "$1" >/dev/null 2>&1; then
        echo "| $label | not installed | $PRETTY_NAME | | |"
        return
    fi
    "$@" >/dev/null 2>&1 &
    pid=$!
    i=0
    while [ $i -lt 40 ] && [ ! -s "$results/$slug.txt" ]; do sleep 0.5; i=$((i + 1)); done
    sleep 0.3
    kill "$pid" 2>/dev/null
    file="$results/$slug.txt"
    if [ ! -s "$file" ]; then
        echo "| $label | $version | $PRETTY_NAME | no result: the terminal did not run the probe | |"
        return
    fi
    ms=$(sed -n 's/^ms=//p' "$file")
    bg=$(sed -n "s/.*TERMINAL_BACKGROUND='\(#[0-9A-F]*\)'.*/\1/p" "$file")
    fg=$(sed -n "s/.*TERMINAL_FOREGROUND='\(#[0-9A-F]*\)'.*/\1/p" "$file")
    scheme=$(sed -n "s/.*TERMINAL_COLOR_SCHEME='\([a-z]*\)'.*/\1/p" "$file")
    if [ -n "$bg" ]; then
        echo "| $label | $version | $PRETTY_NAME | yes ($bg on $fg, $scheme) | $ms ms |"
    else
        echo "| $label | $version | $PRETTY_NAME | no (silent) | $ms ms |"
    fi
}

try xterm "xterm" xterm xterm -e sh "$probe" xterm
try gnome-terminal "GNOME Terminal" gnome-terminal gnome-terminal --wait -- sh "$probe" gnome-terminal
try konsole "Konsole" konsole konsole -e sh "$probe" konsole
try xfce4-terminal "xfce4-terminal" xfce4-terminal xfce4-terminal --disable-server -x sh "$probe" xfce4-terminal
try kitty "kitty" kitty kitty sh "$probe" kitty
try alacritty "Alacritty" alacritty alacritty -e sh "$probe" alacritty
try wezterm "WezTerm" wezterm wezterm start -- sh "$probe" wezterm
try terminator "Terminator" terminator terminator -e "sh '$probe' terminator"
try tilix "Tilix" tilix tilix -e "sh '$probe' tilix"
try urxvt "rxvt-unicode" rxvt-unicode urxvt -e sh "$probe" urxvt
try st "st (suckless)" stterm st -e sh "$probe" st
try lxterminal "LXTerminal" lxterminal lxterminal -e "sh '$probe' lxterminal"
try mate-terminal "MATE Terminal" mate-terminal mate-terminal --disable-factory -e "sh '$probe' mate-terminal"
try qterminal "QTerminal" qterminal qterminal -e "sh '$probe' qterminal"
