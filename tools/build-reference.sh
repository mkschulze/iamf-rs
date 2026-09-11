#!/usr/bin/env bash
# tools/build-reference.sh -- build the IAMF reference decoder at its pinned SHA,
# prove it works on a file we did not write, and stamp what was actually built.
#
# Implements D-11 (split the oracles by build cost: libiamf natively, iamf-tools
# only in a digest-pinned container) and D-13 (GUARD-06 enforced at runtime --
# the manifest records the SHA actually checked out plus a hash of every binary
# produced, and tests/reference_manifest.rs asserts it against REFERENCES.md
# before any conformance clause runs).
#
# This is the phase's tracer: it lights up the FAR end of the chain first. The
# smoke decode below validates the oracle *and* the harness's comparison logic
# against a shipped reference file, before a single byte of our own exists.
#
# Usage:  bash tools/build-reference.sh
# Output: .reference/libiamf/code/test/tools/iamfdec/iamfdec   (gitignored)
#         .reference-manifest.json                             (gitignored)
#
# Requirements: CMake >= 3.6, a C/C++ toolchain, git, python3, and network
# access to github.com for the pinned clone. NOT CMake 3.28, NOT submodules,
# NOT -DIAMF_TEST_TOOL=ON -- none of those exist at v1.1.0; they are
# development-tip facts. See 01-RESEARCH.md "Reference Build Recipes".

set -euo pipefail

# ---------------------------------------------------------------------------
# Pins. A literal SHA, never a tag name and never a branch.
# ---------------------------------------------------------------------------
# A tag can be moved; a branch moves by definition. libiamf's development tip
# has drifted toward the draft-v2.0.0 tree (iamf_obu.c dispatches a Metadata OBU
# case and iamf_obu_raw_is_reserved_obu tests beyond base-enhanced), so a build
# from it judges our output against a different format. Keep the SHA.
LIBIAMF_SHA=f06e919e2ad5502a2adc4bdd4e146f2e7e7ffb63   # tag v1.1.0
LIBIAMF_URL=https://github.com/AOMediaCodec/libiamf.git

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

REF_DIR="$REPO_ROOT/.reference"
SRC="$REF_DIR/libiamf"
PREFIX="$REF_DIR/libiamf-install"
OUT="$REF_DIR/out"
MANIFEST="$REPO_ROOT/.reference-manifest.json"

# iamf_tools_sha is copied from REFERENCES.md rather than re-typed here. This
# script does not build iamf-tools -- that is the digest-pinned container
# (D-11) -- but the manifest must still record which iamf-tools the working
# tree is pinned to, because tests/reference_manifest.rs asserts BOTH SHAs.
IAMF_TOOLS_SHA="$(grep -i 'iamf-tools' REFERENCES.md | grep -oE '\b[0-9a-f]{40}\b' | head -1)"
if [ -z "${IAMF_TOOLS_SHA:-}" ]; then
  echo "FATAL: could not read the iamf-tools SHA out of REFERENCES.md" >&2
  exit 1
fi

log() { printf '\n== %s\n' "$*"; }

NPROC="$( (command -v nproc >/dev/null 2>&1 && nproc) || sysctl -n hw.ncpu 2>/dev/null || echo 4)"

mkdir -p "$REF_DIR" "$OUT"

# ---------------------------------------------------------------------------
# 1. Fetch the pinned tree.
# ---------------------------------------------------------------------------
log "libiamf @ $LIBIAMF_SHA"
if [ ! -d "$SRC/.git" ]; then
  # --filter=blob:none --no-checkout keeps the clone small; the checkout below
  # is by SHA, so no tag or branch name ever selects the tree.
  git clone --filter=blob:none --no-checkout "$LIBIAMF_URL" "$SRC"
fi
if [ "$(git -C "$SRC" rev-parse HEAD 2>/dev/null || echo none)" != "$LIBIAMF_SHA" ]; then
  git -C "$SRC" fetch --quiet origin "$LIBIAMF_SHA" 2>/dev/null || git -C "$SRC" fetch --quiet origin
  git -C "$SRC" checkout -q "$LIBIAMF_SHA"
fi
ACTUAL_SHA="$(git -C "$SRC" rev-parse HEAD)"
if [ "$ACTUAL_SHA" != "$LIBIAMF_SHA" ]; then
  echo "FATAL: checked out $ACTUAL_SHA, expected $LIBIAMF_SHA" >&2
  exit 1
fi
echo "  checked out $ACTUAL_SHA"

# ---------------------------------------------------------------------------
# 2. Select codec support for the host architecture.
# ---------------------------------------------------------------------------
# code/dep_codecs/lib/ ships x86_64-Linux .a files (libopus.a, libFLAC.a,
# libfdk-aac.a) alongside Windows .lib files. CODEC_CAP=ON is the default and
# does find_library(... PATHS dep_codecs/lib NO_DEFAULT_PATH); on a host whose
# architecture does not match those archives -- macOS arm64, for instance --
# CMake *finds* them anyway, compiles the codec sources in, and the link of
# iamfdec then fails with hundreds of
#   ld: symbol(s) not found for architecture x86_64
# Reproduced 2026-09-08. Moving them aside makes the probe fail cleanly, which
# excludes the src/iamf_dec/<codec> source directories. The LPCM path is always
# compiled and needs no codec library at all -- Phase 1 is LPCM-only.
#
# Phase 3's FLAC and Opus conformance fixtures require these shipped
# x86_64-Linux archives. Keep them available on that matching host, which is
# exactly where the reference workflow runs. On every other architecture they
# must remain disabled: CMake would otherwise select an incompatible archive
# and fail at link time before the LPCM smoke test can run.
CODEC_LIB_DIR="$SRC/code/dep_codecs/lib"
CODEC_DISABLED_DIR="$SRC/code/dep_codecs/lib_disabled"
mkdir -p "$CODEC_LIB_DIR" "$CODEC_DISABLED_DIR"

if [ "$(uname -s)" = "Linux" ] && [ "$(uname -m)" = "x86_64" ]; then
  log "enabling bundled FLAC/Opus codec archives (x86_64 Linux reference host)"
  restored=0
  for f in "$CODEC_DISABLED_DIR"/*.a "$CODEC_DISABLED_DIR"/*.lib; do
    [ -e "$f" ] || continue
    mv "$f" "$CODEC_LIB_DIR/"
    restored=$((restored + 1))
  done
  echo "  restored $restored archive(s) (0 on a clean checkout is expected)"
  DEP_CODECS_DISABLED=false
else
  log "disabling bundled codec archives (unsupported reference host)"
  moved=0
  for f in "$CODEC_LIB_DIR"/*.a "$CODEC_LIB_DIR"/*.lib; do
    [ -e "$f" ] || continue
    mv "$f" "$CODEC_DISABLED_DIR/"
    moved=$((moved + 1))
  done
  echo "  moved $moved archive(s) (0 on a re-run is expected)"
  DEP_CODECS_DISABLED=true
fi

# ---------------------------------------------------------------------------
# 3. Build libiamf, then iamfdec (a SEPARATE CMake project at v1.1.0).
# ---------------------------------------------------------------------------
# BUILD_SHARED_LIBS=OFF so iamfdec needs no DYLD_LIBRARY_PATH / LD_LIBRARY_PATH
# at test time -- the harness invokes it by absolute path from cargo.
CONFIGURE_LOG="$REF_DIR/configure.log"
log "cmake configure + build (libiamf)"
(
  cd "$SRC/code"
  cmake -DCMAKE_INSTALL_PREFIX="$PREFIX" -DBUILD_SHARED_LIBS=OFF . 2>&1 | tee "$CONFIGURE_LOG"
  make -j"$NPROC"
  make install
) > "$REF_DIR/build-libiamf.log" 2>&1

if [ "$DEP_CODECS_DISABLED" = true ]; then
  # These lines confirm the LPCM-only fallback used on non-Linux hosts.
  log "codec-exclusion confirmation (expected: opus, fdk-aac, FLAC not found)"
  grep -iE 'the (opus|fdk-aac|FLAC) library was not found' "$CONFIGURE_LOG" \
    || echo "  WARNING: the configure log did not carry the codec-not-found lines; see $CONFIGURE_LOG"
else
  log "codec-enable confirmation (FLAC and Opus must be configured)"
  if grep -qiE 'the (opus|FLAC) library was not found' "$CONFIGURE_LOG"; then
    echo "FATAL: the Linux reference build did not configure FLAC and Opus support" >&2
    exit 1
  fi
fi

log "cmake configure + build (iamfdec)"
if ! (
  cd "$SRC/code/test/tools/iamfdec"
  cmake -DCMAKE_INSTALL_PREFIX="$PREFIX" .
  make -j"$NPROC"
) > "$REF_DIR/build-iamfdec.log" 2>&1; then
  echo "FATAL: iamfdec build failed; showing $REF_DIR/build-iamfdec.log" >&2
  cat "$REF_DIR/build-iamfdec.log" >&2
  exit 1
fi

IAMFDEC="$SRC/code/test/tools/iamfdec/iamfdec"
LIBIAMF_A="$(find "$PREFIX" "$SRC/code" -name 'libiamf.a' -print 2>/dev/null | head -1)"
[ -x "$IAMFDEC" ] || { echo "FATAL: iamfdec was not produced at $IAMFDEC" >&2; exit 1; }
[ -n "$LIBIAMF_A" ] || { echo "FATAL: libiamf.a was not produced" >&2; exit 1; }
echo "  iamfdec:    $IAMFDEC"
echo "  libiamf.a:  $LIBIAMF_A"

# ---------------------------------------------------------------------------
# 4. The self-validating smoke test -- the tracer's real verify.
# ---------------------------------------------------------------------------
# Decode a file WE DID NOT WRITE and compare it to the source PCM that shipped
# beside it. This proves the binary works AND that the comparison logic works,
# before any of our own bytes exist.
#
# Every flag is load-bearing:
#   -r 16000          iamfdec DEFAULTS to 48000 and drives the speex resampler;
#                     test_000003 is 16 kHz, so the default silently resamples.
#   -s0               Sound System A (0+2+0) -- that file's layout. (-s1 = 5.1.)
#   -d 16             output WAV bit depth == the file's sample_size, so the
#                     comparison is like-for-like.
#   -disable_limiter  IAMF_decoder_open() UNCONDITIONALLY creates a -1 dBTP peak
#                     limiter. Below threshold its gain is exactly 1.0 and the
#                     path is bit-exact, but -disable_limiter destroys the
#                     limiter outright and removes the 240-sample look-ahead
#                     path entirely. Do not "raise the amplitude to make the
#                     signal clearer" in a later fixture without re-reading this.
#
# DO NOT TREAT EXIT CODE 0 AS SUCCESS. iamfdec prints "<path> can't opened." to
# stderr and returns 0 when -o3 is handed a directory, and a file with
# num_samples_per_frame = 0 decodes to ZERO samples and also returns 0. The
# three signals below -- the output file exists and exceeds a bare WAV header,
# the frame count, the differing-sample count -- are the assertions.
log "smoke decode: test_000003.iamf vs sawtooth_100_stereo.wav"
SMOKE_IAMF="$SRC/tests/test_000003.iamf"
SMOKE_REF_WAV="$SRC/tests/sawtooth_100_stereo.wav"
SMOKE_OUT="$OUT/t3.wav"
SMOKE_EXPECT_FRAMES=8000
SMOKE_EXPECT_TOTAL=16000
rm -f "$SMOKE_OUT"

set +e
"$IAMFDEC" -i0 -o3 "$SMOKE_OUT" -r 16000 -s0 -d 16 -disable_limiter "$SMOKE_IAMF" \
  > "$REF_DIR/smoke.log" 2>&1
SMOKE_EXIT=$?
set -e
echo "  iamfdec exit status: $SMOKE_EXIT (informational only -- NOT the signal)"

python3 - "$SMOKE_OUT" "$SMOKE_REF_WAV" "$SMOKE_EXPECT_FRAMES" "$SMOKE_EXPECT_TOTAL" <<'PY'
import os, struct, sys, wave

out_path, ref_path = sys.argv[1], sys.argv[2]
want_frames, want_total = int(sys.argv[3]), int(sys.argv[4])

# Assertion 1: the output exists and is larger than a bare 44-byte WAV header.
if not os.path.exists(out_path) or os.path.getsize(out_path) <= 44:
    print("  output file missing or too small")
    sys.exit(1)

def read_pcm16(path):
    with wave.open(path, "rb") as w:
        return w.getnchannels(), w.getnframes(), w.getsampwidth(), w.readframes(w.getnframes())

och, ofr, osw, odata = read_pcm16(out_path)
_rch, _rfr, _rsw, rdata = read_pcm16(ref_path)

print(f"  channels {och}  frames {ofr}  bit depth {osw * 8}")

# Assertion 2: the frame count. A "successful" decode can yield zero samples.
if ofr != want_frames:
    print(f"  frame count {ofr}, expected {want_frames}")
    sys.exit(1)

# Assertion 3: the differing-sample count over the full interleaved buffer.
n = min(len(odata), len(rdata)) // 2
o = struct.unpack("<%dh" % n, odata[: n * 2])
r = struct.unpack("<%dh" % n, rdata[: n * 2])
diff = sum(1 for a, b in zip(o, r) if a != b)
total = och * ofr
print(f"  differing samples: {diff} of {total}")

if total != want_total:
    print(f"  total sample count {total}, expected {want_total}")
    sys.exit(1)
if diff != 0:
    print(f"  differing-sample count {diff}, expected 0")
    sys.exit(1)
print("  smoke test PASSED -- oracle and comparison logic both proven")
PY

# ---------------------------------------------------------------------------
# 5. Stamp the manifest (D-13).
# ---------------------------------------------------------------------------
sha256_of() {
  if command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | awk '{print $1}'
  else sha256sum "$1" | awk '{print $1}'; fi
}

HOST_TRIPLE="$(rustc -vV 2>/dev/null | awk '/^host:/{print $2}')"
[ -n "${HOST_TRIPLE:-}" ] || HOST_TRIPLE="$(uname -m)-unknown-$(uname -s | tr '[:upper:]' '[:lower:]')"
CMAKE_VERSION="$(cmake --version | head -1 | awk '{print $3}')"

# `built_at` is informational only. tests/reference_manifest.rs asserts on the
# SHAs, and the re-runnability check compares iamfdec_sha256 across two runs --
# neither reads a timestamp, so re-running does not perturb either.
cat > "$MANIFEST" <<EOF
{
  "libiamf_sha": "$ACTUAL_SHA",
  "iamf_tools_sha": "$IAMF_TOOLS_SHA",
  "libiamf_a_sha256": "$(sha256_of "$LIBIAMF_A")",
  "iamfdec_sha256": "$(sha256_of "$IAMFDEC")",
  "iamfdec_path": "$IAMFDEC",
  "host_triple": "$HOST_TRIPLE",
  "cmake_version": "$CMAKE_VERSION",
  "dep_codecs_disabled": $DEP_CODECS_DISABLED,
  "smoke_frames": $SMOKE_EXPECT_FRAMES,
  "smoke_differing_samples": 0,
  "built_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
}
EOF

log "manifest written: $MANIFEST"
cat "$MANIFEST"

cat <<EOF

== Reference decoder ready.

Paste this to run the reference-gated tests:

  export IAMF_REF_DECODER=$IAMFDEC

With it unset, tests/reference_manifest.rs skips with a printed reason and
\`cargo test\` stays green offline on all four targets (CONF-10).
EOF
