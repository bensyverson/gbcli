#!/bin/bash
# Builds unpatched RGBDS 1.0.4 rgbasm/rgblink/rgbfix to wasm with Emscripten, no CMake
# (web spike, leaf LMjWWK; project/2026-11-09-web-spike-rgbds.md).
# Usage: build-rgbds-wasm.sh <rgbds-1.0.4 source dir> <out dir>
# Needs em++ (Homebrew emscripten; built with 7.0.0) and Homebrew bison.
# Check the result: node scripts/web/test-rgbds-wasm.mjs <out dir>
set -eu
SRC=$1; OUT=$2; BISON=/opt/homebrew/opt/bison/bin/bison
mkdir -p "$OUT/obj"
COMMON="cli diagnostics style usage util version extern/getopt"
OPT="-O3 -flto -std=c++20 -DNDEBUG -I$SRC/include -I$OUT -DBUILD_VERSION_STRING=\"v1.0.4\""
LD="-sALLOW_MEMORY_GROWTH=1 -sMODULARIZE=1 -sEXPORT_ES6=1 -sENVIRONMENT=web,node -sEXPORTED_RUNTIME_METHODS=FS -sEXIT_RUNTIME=1"
FLAGS="-Dlr.type=ielr -Dparse.lac=full -Dapi.token.raw=true -Dparse.error=detailed"
$BISON $FLAGS --header="$OUT/parser.hpp" -o "$OUT/parser.cpp" "$SRC/src/asm/parser.y"
$BISON $FLAGS --header="$OUT/script.hpp" -o "$OUT/script.cpp" "$SRC/src/link/script.y"
build() { # name, sources...
  name=$1; shift; objs=""
  for s in "$@"; do
    case $s in /*) f=$s;; *) f=$SRC/src/$s.cpp;; esac
    o=$OUT/obj/$name-$(echo "$s" | tr '/' '_' ).o
    em++ $OPT -c "$f" -o "$o" -I"$SRC/src" ; objs="$objs $o"
  done
  em++ $OPT $LD -sEXPORT_NAME=create_$name $objs -o "$OUT/$name.js"
}
build rgbasm $COMMON "$OUT/parser.cpp" asm/actions asm/charmap asm/fixpoint asm/format asm/fstack asm/intern asm/lexer asm/macro asm/main asm/opt asm/output asm/rpn asm/section asm/symbol asm/warning extern/utf8decoder backtrace linkdefs opmath verbosity
build rgblink $COMMON "$OUT/script.cpp" link/assign link/fstack link/lexer link/layout link/main link/object link/output link/patch link/sdas_obj link/section link/symbol link/warning extern/utf8decoder backtrace linkdefs opmath verbosity
build rgbfix $COMMON fix/fix fix/main fix/mbc fix/warning
