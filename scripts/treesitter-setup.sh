#!/usr/bin/env bash
# Hado の tree-sitter 門番が使う道具（tree-sitter CLI と Neovim）を、固定した版と
# checksum で取ってくる setup script（docs/issues/highlight-treesitter.md）。
#
# 標準環境の PATH にある `tree-sitter` や `nvim` は使わない。取得した実体の絶対
# パスだけを stdout に出し、門番はそのパスを使う。版と checksum は下の定数が正で、
# 取得のたびに照合する。
#
#   bash scripts/treesitter-setup.sh [--cache DIR]
#
# stdout（2 行）:
#   tree-sitter /abs/path/to/tree-sitter
#   nvim /abs/path/to/nvim
#
# `--cache DIR` を省くと `HADO_TREESITTER_CACHE`、それも無ければ `mktemp -d` を
# 使う。取得と展開の進み具合は stderr に出す。
set -euo pipefail

TREE_SITTER_VERSION=0.27.0
NVIM_VERSION=0.12.5

# tree-sitter/tree-sitter v0.27.0 の release asset。
TREE_SITTER_SHA256_X64=20a1f39ec1c45f2211492dcb8881c802b643b554bb196869a29ac3778277fa77
TREE_SITTER_SHA256_ARM64=3a35a2dd961ad842384e982c75daf792c01d1a67e442fc3914d4de37bd8a59cb
# neovim/neovim v0.12.5 の release asset。
NVIM_SHA256_X86_64=bce0f56eda1f1b1db6eee8f4133d7a38813ea07933837dd1777411ca384c6875
NVIM_SHA256_ARM64=1aa5ca085249580ae0f91eb14f27ec0919773ff2d99a163d03f3d6c21ac29725

cache=
while [ "$#" -gt 0 ]; do
    case "$1" in
        --cache)
            [ "$#" -ge 2 ] || { echo 'treesitter-setup: --cache needs a directory' >&2; exit 1; }
            cache=$2
            shift 2
            ;;
        *)
            echo "treesitter-setup: unknown option: $1" >&2
            exit 1
            ;;
    esac
done
if [ -z "$cache" ]; then
    cache=${HADO_TREESITTER_CACHE:-}
fi
if [ -z "$cache" ]; then
    cache=$(mktemp -d "${TMPDIR:-/tmp}/hado-treesitter-XXXXXX")
fi
mkdir -p -- "$cache"
cache=$(cd -- "$cache" && pwd)

log() { echo "treesitter-setup: $*" >&2; }

case "$(uname -s)" in
    Linux) ;;
    *)
        echo "treesitter-setup: only Linux has pinned assets" >&2
        exit 1
        ;;
esac
case "$(uname -m)" in
    x86_64 | amd64)
        ts_arch=x64
        nvim_arch=x86_64
        ts_sha=$TREE_SITTER_SHA256_X64
        nvim_sha=$NVIM_SHA256_X86_64
        ;;
    aarch64 | arm64)
        ts_arch=arm64
        nvim_arch=arm64
        ts_sha=$TREE_SITTER_SHA256_ARM64
        nvim_sha=$NVIM_SHA256_ARM64
        ;;
    *)
        echo "treesitter-setup: no pinned asset for $(uname -m)" >&2
        exit 1
        ;;
esac

fetch() {
    # fetch <url> <destination> <sha256>
    local url=$1 dest=$2 want=$3 have
    if [ -f "$dest" ]; then
        have=$(sha256sum -- "$dest" | cut -d' ' -f1)
        if [ "$have" = "$want" ]; then
            log "cached ${dest##*/}"
            return 0
        fi
        log "checksum mismatch for cached ${dest##*/}; refetching"
        rm -f -- "$dest"
    fi
    log "fetching ${url##*/}"
    curl -fsSL --retry 3 -o "$dest.part" -- "$url"
    have=$(sha256sum -- "$dest.part" | cut -d' ' -f1)
    if [ "$have" != "$want" ]; then
        rm -f -- "$dest.part"
        echo "treesitter-setup: checksum mismatch for ${url##*/}: $have" >&2
        exit 1
    fi
    mv -- "$dest.part" "$dest"
}

ts_archive="$cache/tree-sitter-linux-$ts_arch.gz"
nvim_archive="$cache/nvim-linux-$nvim_arch.tar.gz"
fetch "https://github.com/tree-sitter/tree-sitter/releases/download/v$TREE_SITTER_VERSION/tree-sitter-linux-$ts_arch.gz" \
    "$ts_archive" "$ts_sha"
fetch "https://github.com/neovim/neovim/releases/download/v$NVIM_VERSION/nvim-linux-$nvim_arch.tar.gz" \
    "$nvim_archive" "$nvim_sha"

ts_bin="$cache/tree-sitter-$TREE_SITTER_VERSION"
if [ ! -x "$ts_bin" ]; then
    log "unpacking tree-sitter $TREE_SITTER_VERSION"
    gunzip -c -- "$ts_archive" >"$ts_bin.part"
    chmod +x "$ts_bin.part"
    mv -- "$ts_bin.part" "$ts_bin"
fi

nvim_dir="$cache/nvim-linux-$nvim_arch"
if [ ! -x "$nvim_dir/bin/nvim" ]; then
    log "unpacking Neovim $NVIM_VERSION"
    rm -rf -- "$nvim_dir.part"
    mkdir -p -- "$nvim_dir.part"
    tar xzf "$nvim_archive" -C "$nvim_dir.part" --strip-components=1
    rm -rf -- "$nvim_dir"
    mv -- "$nvim_dir.part" "$nvim_dir"
fi
nvim_bin="$nvim_dir/bin/nvim"

got_ts=$("$ts_bin" --version | awk '{print $2}')
if [ "$got_ts" != "$TREE_SITTER_VERSION" ]; then
    echo "treesitter-setup: tree-sitter $TREE_SITTER_VERSION expected, got $got_ts" >&2
    exit 1
fi
# `nvim --version` は state ディレクトリに log を書く。書き込み先を cache の下へ
# 向け、カレントに nvim.log を残さない。
got_nvim=$(cd "$cache" && XDG_STATE_HOME="$cache/xdg-state" \
    XDG_DATA_HOME="$cache/xdg-data" XDG_CONFIG_HOME="$cache/xdg-config" \
    XDG_CACHE_HOME="$cache/xdg-cache" NVIM_LOG_FILE="$cache/nvim.log" \
    "$nvim_bin" --version | awk 'NR==1 {print $2}')
got_nvim=${got_nvim#v}
if [ "$got_nvim" != "$NVIM_VERSION" ]; then
    echo "treesitter-setup: Neovim $NVIM_VERSION expected, got $got_nvim" >&2
    exit 1
fi

printf 'tree-sitter %s\n' "$ts_bin"
printf 'nvim %s\n' "$nvim_bin"
