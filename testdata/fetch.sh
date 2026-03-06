#!/usr/bin/env bash
# download test edf/bdf files from teuniz.net
set -euo pipefail

cd "$(dirname "$0")"

base="https://www.teuniz.net/edf_bdf_testfiles"

fetch() {
  local url="$1" zip="$2"
  if ls "${zip%.zip}".* 1>/dev/null 2>&1; then
    echo "already have $zip contents, skipping"
    return
  fi
  echo "fetching $zip"
  curl -sL -o "$zip" "$url/$zip"
  python3 -c "import zipfile; zipfile.ZipFile('$zip').extractall('.')"
  rm -f "$zip"
}

fetch "$base" test_generator.zip
fetch "$base" test_generator_2_bdfplus.zip

echo "done"
ls -lh ./*.edf ./*.bdf 2>/dev/null
