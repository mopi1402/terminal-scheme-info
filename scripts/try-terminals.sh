#!/bin/sh
# Runs one real query in every installed terminal emulator (macOS) and prints
# a markdown table (terminal, version, OS, answer, time).
#
# Each terminal is launched with ZDOTDIR pointing at a throwaway .zshrc that
# runs scripts/probe-terminal.zsh once and exits, so no terminal needs a
# specific command-line flag. Results land in /tmp/tsi-terminals/<name>.txt.
# Windows flash open and close. Terminals already running are left running.
#
#   scripts/try-terminals.sh              # every known terminal
#   scripts/try-terminals.sh kitty rio    # a selection
set -u
cd "$(dirname "$0")/.." || exit 1
zdotdir=/tmp/tsi-zdotdir
results=/tmp/tsi-terminals
rm -rf "$results"
mkdir -p "$zdotdir" "$results"
cat > "$zdotdir/.zshrc" <<ZSHRC
zsh '$PWD/scripts/probe-terminal.zsh' "\$TSI_NAME"
exit
ZSHRC
os="macOS $(sw_vers -productVersion)"

echo "| Terminal | Version | OS | Answers OSC 10/11 | Time |"
echo "| --- | --- | --- | --- | --- |"

try() { # try NAME APP
    name=$1 app=$2
    path=$(mdfind "kMDItemKind == 'Application' && kMDItemDisplayName == '$app.app'" 2>/dev/null | head -1)
    [ -n "$path" ] || path=$(find /Applications /System/Applications "$HOME/Applications" -maxdepth 2 -name "$app.app" 2>/dev/null | head -1)
    if [ -z "$path" ]; then
        echo "| $app | not installed | $os | | |"
        return
    fi
    version=$(defaults read "$path/Contents/Info" CFBundleShortVersionString 2>/dev/null)
    if pgrep -qf "/$app.app/"; then was_running=yes; new=-n; else was_running=no; new=; fi
    if ! open $new -a "$app" --env ZDOTDIR="$zdotdir" --env TSI_NAME="$name"; then
        echo "$name: open failed"
        return
    fi
    i=0
    while [ $i -lt 40 ] && [ ! -s "$results/$name.txt" ]; do sleep 0.5; i=$((i + 1)); done
    sleep 0.5
    [ "$was_running" = no ] && osascript -e "quit app \"$app\"" >/dev/null 2>&1
    file="$results/$name.txt"
    if [ ! -s "$file" ]; then
        echo "| $app | $version | $os | no result: the terminal did not run the probe | |"
        return
    fi
    ms=$(sed -n 's/^ms=//p' "$file")
    bg=$(sed -n "s/.*TERMINAL_BACKGROUND='\(#[0-9A-F]*\)'.*/\1/p" "$file")
    fg=$(sed -n "s/.*TERMINAL_FOREGROUND='\(#[0-9A-F]*\)'.*/\1/p" "$file")
    scheme=$(sed -n "s/.*TERMINAL_COLOR_SCHEME='\([a-z]*\)'.*/\1/p" "$file")
    if [ -n "$bg" ]; then
        echo "| $app | $version | $os | yes ($bg on $fg, $scheme) | $ms ms |"
    else
        echo "| $app | $version | $os | no (silent) | $ms ms |"
    fi
}

selected=${*:-kitty wezterm alacritty rio contour hyper tabby wave terminal iterm2}
for name in $selected; do
    case $name in
        kitty) try kitty kitty ;;
        wezterm) try wezterm WezTerm ;;
        alacritty) try alacritty Alacritty ;;
        rio) try rio rio ;;
        contour) try contour contour ;;
        hyper) try hyper Hyper ;;
        tabby) try tabby Tabby ;;
        wave) try wave Wave ;;
        terminal) try terminal Terminal ;;
        iterm2) try iterm2 iTerm ;;
        ghostty) try ghostty Ghostty ;;
        *) echo "unknown terminal: $name" ;;
    esac
done
