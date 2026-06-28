#!/usr/bin/env bash
# Encode a token for embedding in the media plugin binary.
# XORs each byte against the key "NXOR" and prints the result as hex.
# Usage: scripts/xor-token.sh <token>
# Output: pass the printed hex string to go build -ldflags "-X main.bundledTMDbTokenHex=<hex>"
set -euo pipefail

if [[ $# -ne 1 || -z "$1" ]]; then
    echo "Usage: $0 <token>" >&2
    exit 1
fi

token="$1"
key="NXOR"
klen=${#key}
result=""

for (( i=0; i<${#token}; i++ )); do
    tb=$(printf '%d' "'${token:$i:1}")
    kb=$(printf '%d' "'${key:$(( i % klen )):1}")
    xb=$(( tb ^ kb ))
    result+=$(printf '%02x' "$xb")
done

echo "$result"
