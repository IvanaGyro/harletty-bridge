#!/usr/bin/env bash
# Download the real TrueHD samples the Media Foundation decoder's tests play
# (never committed) into DEST. Each file is tried several times, with a long
# connect timeout, because the hosts are small and sometimes slow to answer.
#   scripts/fetch-truehd-samples.sh DEST
set -euo pipefail
dest="${1:?usage: fetch-truehd-samples.sh DEST}"
mkdir -p "$dest"

fetch() { # name url
  local name="$1" url="$2" attempt
  if [ -s "$dest/$name" ]; then
    echo "$name: already there"
    return 0
  fi
  for attempt in 1 2 3 4 5 6; do
    if curl -fsSL --connect-timeout 30 --max-time 300 -o "$dest/$name.part" "$url"; then
      mv "$dest/$name.part" "$dest/$name"
      return 0
    fi
    echo "$name: attempt $attempt failed" >&2
    sleep $((attempt * 10))
  done
  echo "$name: gave up on $url" >&2
  return 1
}

fetch atmos.thd https://fate-suite.ffmpeg.org/truehd/atmos.thd
fetch ffvii.mkv 'https://samples.ffmpeg.org/A-codecs/TrueHD/TrueHD%20Sample%201%20%28Final%20Fantasy%20VII%20Advent%20Children%29.mkv'
ls -l "$dest"
