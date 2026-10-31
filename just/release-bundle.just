# Create release archive(s) for the current platform.
# Arguments arrive as "$1".."$4" rather than being spliced into the command line,
# so values are never parsed as shell syntax.

# `version` should be a tag like v0.30.0.
[unix]
[positional-arguments]
release-bundle version output="dist" flavor="" arch="":
    @MESH_RELEASE_FLAVOR="$3" MESH_RELEASE_ARCH="$4" scripts/package-release.sh "$1" "$2"

[windows]
[positional-arguments]
release-bundle version output="dist" flavor="":
    @powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-release.ps1 -Version "$1" -OutputDir "$2" -Flavor "$3"

# Run repo-level release-target consistency checks.
[unix]
check-release:
    just with-lld cargo xtool repo-consistency release-targets

[windows]
check-release:
    @just with-lld cargo xtool repo-consistency release-targets

# Check the Rust process-environment mutation census and contract annotations.
check-env-mutation-contract:
    @just with-lld cargo xtool repository env-mutation-census

# Run one release-attestation xtask action through the repository toolchain.
release-attestation *ARGS:
    @just with-lld cargo run -q -p xtask -- release-attestation {{ ARGS }}

# Collect raw release evidence; Unix positional forwarding preserves every argument.
[unix]
[positional-arguments]
release-inventory *ARGS:
    @just automation-run release inventory "$@"
