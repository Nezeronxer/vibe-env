#!/bin/sh
# Same capture as mockup/shoot.sh (headless Chrome, 1440x900, scale 1) for the real UI on dev-mock.
# ./shoot.sh name:'query-string' [name:query ...] -> <name>-<theme>.png in both themes; WIDTH=1000 ./shoot.sh ... for the narrow window.
cd "$(dirname "$0")"
UI="$PWD/../../../ui/index.html"
C="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"
W="${WIDTH:-1440}"
for a in "$@"; do
  n="${a%%:*}"; q="${a#*:}"
  for th in dark light; do
    "$C" --headless --disable-gpu --hide-scrollbars --force-device-scale-factor=1 --virtual-time-budget=4000 --screenshot="$PWD/$n-$th.png" --window-size=$W,900 "file://$UI?theme=$th&lang=ru&$q" >/dev/null 2>&1
  done
done
