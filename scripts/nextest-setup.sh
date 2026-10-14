#!/usr/bin/env bash
# 門番の全体テストが使う cargo-nextest を、固定した版と checksum で取ってくる setup
# script（docs/rounds/nextest.md）。
#
# PATH にある `cargo-nextest` は使わない。取得した実体の絶対パスだけを stdout に出す。
# 版と checksum は下の定数が正で、取得のたびに照合する。
#
#   bash scripts/nextest-setup.sh [--cache DIR]
#
# `--cache DIR` を省くと `HADO_NEXTEST_CACHE`、それも無ければ `mktemp -d` を使う。
# 取得と展開の進み具合は stderr に出す。
set -euo pipefail

NEXTEST_VERSION=0.9.148

# nextest-rs/nextest cargo-nextest-0.9.148 の release asset（GitHub の digest）。
NEXTEST_SHA256_X86_64=223ba936714cc861a9cdedad5a8eb664dfdd69d6fbdf6cea6aa8184f523290dc
NEXTEST_SHA256_AARCH64=81469eb963ef883fd590e60189e77d6aba3d91b418591d3391f50fe3e9480d2b

cache=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --cache)
            [ "$#" -ge 2 ] || { echo 'nextest-setup: --cache needs a directory' >&2; exit 1; }
            cache=$2
            shift 2
            ;;
        *)
            echo "nextest-setup: unknown option: $1" >&2
            exit 1
            ;;
    esac
done
if [ -z "$cache" ]; then
    cache=${HADO_NEXTEST_CACHE:-}
fi
if [ -z "$cache" ]; then
    cache=$(mktemp -d "${TMPDIR:-/tmp}/hado-nextest-XXXXXX")
fi
mkdir -p -- "$cache"
cache=$(cd -- "$cache" && pwd)

log() { echo "nextest-setup: $*" >&2; }

case "$(uname -s)" in
    Linux) ;;
    *)
        echo "nextest-setup: only Linux has pinned assets" >&2
        exit 1
        ;;
esac
case "$(uname -m)" in
    x86_64 | amd64)
        arch=x86_64
        sha=$NEXTEST_SHA256_X86_64
        ;;
    aarch64 | arm64)
        arch=aarch64
        sha=$NEXTEST_SHA256_AARCH64
        ;;
    *)
        echo "nextest-setup: no pinned asset for $(uname -m)" >&2
        exit 1
        ;;
esac

asset="cargo-nextest-$NEXTEST_VERSION-$arch-unknown-linux-gnu.tar.gz"
archive="$cache/$asset"
url="https://github.com/nextest-rs/nextest/releases/download/cargo-nextest-$NEXTEST_VERSION/$asset"
if [ -f "$archive" ] && [ "$(sha256sum -- "$archive" | cut -d' ' -f1)" = "$sha" ]; then
    log "cached $asset"
else
    log "fetching $asset"
    curl -fsSL --retry 3 -o "$archive.part" -- "$url"
    have=$(sha256sum -- "$archive.part" | cut -d' ' -f1)
    if [ "$have" != "$sha" ]; then
        rm -f -- "$archive.part"
        echo "nextest-setup: checksum mismatch for $asset: $have" >&2
        exit 1
    fi
    mv -- "$archive.part" "$archive"
fi

bin="$cache/cargo-nextest-$NEXTEST_VERSION"
if [ ! -x "$bin" ]; then
    log "unpacking cargo-nextest $NEXTEST_VERSION"
    rm -rf -- "$bin.part"
    mkdir -p -- "$bin.part"
    tar xzf "$archive" -C "$bin.part"
    mv -- "$bin.part/cargo-nextest" "$bin"
    rm -rf -- "$bin.part"
fi

got=$("$bin" nextest --version | awk 'NR==1 {print $2}')
if [ "$got" != "$NEXTEST_VERSION" ]; then
    echo "nextest-setup: cargo-nextest $NEXTEST_VERSION expected, got $got" >&2
    exit 1
fi

printf '%s\n' "$bin"
