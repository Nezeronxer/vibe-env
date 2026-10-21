#!/bin/sh
# ./shoot.sh имя:хеш ... -> shots/имя.png
cd "$(dirname "$0")"
C="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
for a in "$@"; do
  n="${a%%:*}"; h="${a#*:}"
  "$C" --headless --disable-gpu --hide-scrollbars --force-device-scale-factor=1 --virtual-time-budget=1500 --screenshot="$PWD/shots/$n.png" --window-size=1440,900 "file://$PWD/mockup.html#$h" >/dev/null 2>&1
done
