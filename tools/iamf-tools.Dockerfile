# tools/iamf-tools.Dockerfile
#
# The second reference oracle (D-11). `iamf-tools` needs Bazel + abseil +
# protobuf + fdk-aac, which is not a thing to run on every save, so it lives in
# a container while `libiamf` is built natively by tools/build-reference.sh.
#
# It serves CONF-06 (iamf-tools' own parser accepts our file, through
# decoder_main's parse path) and CONF-07 (produce a companion file for our own
# configuration and byte-diff it). It does NOT serve fixture supply -- see
# tests/fixtures/MANIFEST.md, research correction 6: 221 .iamf files are already
# committed under BSD-3-Clause-Clear and need no Bazel run at all.
#
# Everything that can move is pinned, because GUARD-06 exists to stop exactly
# this file from quietly becoming a different oracle:
#   * the base image, by @sha256 digest, not by tag;
#   * the iamf-tools tree, by commit SHA, not by tag and never by branch;
#   * Bazelisk, by release version AND by the sha256 of the binary itself;
#   * Bazel itself, by iamf-tools' own committed .bazelversion (7.4.1) -- which
#     is why no Bazel version appears here: pinning it in two places would let
#     the two drift, and the tree's own file is the authority.
#
# D-13 additionally requires the RESULTING image be pinned by digest. Build it
# in CI (Linux x64), push it, and record the built image's digest in
# REFERENCES.md alongside the base digest below. The base digest alone does not
# pin the result -- apt and the Bazel fetch both reach the network.
#
# Build:  docker build -f tools/iamf-tools.Dockerfile -t iamf-tools:v2.1.0 tools/
# Run:    docker run --rm -v "$PWD:/work" iamf-tools:v2.1.0 \
#           bazel-bin/iamf/cli/decoder_main --input_filename=/work/... \
#                                           --output_filename=/work/...

# ubuntu:24.04 multi-arch index digest, verified against registry-1.docker.io
# (HTTP 200, docker-content-digest echoed back) on 2026-09-08.
FROM ubuntu@sha256:33ceb71981b602c1a7443a53469e4dba065f7503eab3078a2d7a57a2ab987517

# tag v2.1.0. NOT the development tip: that tree models draft-v2.0.0 (a Metadata
# OBU at type 24, profiles beyond base-enhanced, object-based Audio Elements),
# and a file measured against it is a file libiamf@v1.1.0 rejects.
ARG IAMF_TOOLS_SHA=848c6ff4968ff8cc6f728259892ab4f90cb83256

# A versioned release, never `latest/download`. `latest` is precisely the moving
# target GUARD-06 exists to prevent -- it would silently change the oracle
# between two runs of the same commit.
ARG BAZELISK_VERSION=v1.29.0
ARG BAZELISK_SHA256_AMD64=5a408715e932c0250d28bd84555f12edbf70117de42f9181691c736eacc4a992
ARG BAZELISK_SHA256_ARM64=e20e8b0f4f240091b7a55bf17b9398bd4f40ee70ae0208dff95dd4c445fb4010

# Supplied automatically by BuildKit. Handling both arches means an Apple
# Silicon developer can build the image locally without --platform=linux/amd64
# and its emulation penalty; CI still builds linux/amd64, which is the digest
# that gets recorded.
ARG TARGETARCH

ENV DEBIAN_FRONTEND=noninteractive

# From iamf-tools@v2.1.0 docs/build_instructions.md: Bazelisk, CMake (some
# dependencies build with it), and Clang 13+ or GCC 10+. python3/zip/unzip are
# Bazel's own runtime requirements. Research recorded this package set as
# ASSUMED (assumption A2); if a build fails for a missing package, add it HERE
# and say so in the commit message rather than installing it at run time.
RUN apt-get update && apt-get install -y --no-install-recommends \
      ca-certificates \
      git \
      curl \
      build-essential \
      clang \
      cmake \
      python3 \
      unzip \
      zip \
    && rm -rf /var/lib/apt/lists/*

RUN set -eux; \
    case "${TARGETARCH:-amd64}" in \
      amd64) arch=amd64; sha="${BAZELISK_SHA256_AMD64}" ;; \
      arm64) arch=arm64; sha="${BAZELISK_SHA256_ARM64}" ;; \
      *) echo "unsupported TARGETARCH=${TARGETARCH}" >&2; exit 1 ;; \
    esac; \
    curl -fsSL -o /usr/local/bin/bazel \
      "https://github.com/bazelbuild/bazelisk/releases/download/${BAZELISK_VERSION}/bazelisk-linux-${arch}"; \
    echo "${sha}  /usr/local/bin/bazel" | sha256sum -c -; \
    chmod +x /usr/local/bin/bazel

WORKDIR /src

# --filter=blob:none --no-checkout, then checkout by SHA: no tag or branch name
# ever selects the tree.
RUN set -eux; \
    git clone --filter=blob:none --no-checkout \
      https://github.com/AOMediaCodec/iamf-tools.git .; \
    git checkout -q "${IAMF_TOOLS_SHA}"; \
    test "$(git rev-parse HEAD)" = "${IAMF_TOOLS_SHA}"

# Bazelisk reads /src/.bazelversion (7.4.1 at this SHA) and fetches that exact
# Bazel. This is the layer that costs the wall-clock: abseil, protobuf, fdk-aac
# and the rest are fetched and compiled here.
#
# Only two cc_binary targets exist at v2.1.0. There is no probe_main -- STACK.md
# says there is, and it is wrong (research correction 8). CONF-06 therefore runs
# through decoder_main's parse path (ObuProcessor / DescriptorObuParser), which
# is the strict parser; see CONFORMANCE-GATE.md.
RUN bazel build -c opt //iamf/cli:encoder_main //iamf/cli:decoder_main

# A build-time assertion, so a broken image fails at `docker build` rather than
# at the first conformance run.
RUN test -x bazel-bin/iamf/cli/encoder_main && test -x bazel-bin/iamf/cli/decoder_main

CMD ["bash"]
