# Start a lite client — no GPU, no model, just a local HTTP proxy to the mesh host.
# Arguments arrive as "$1".."$4" rather than being spliced into the script, so a
# pasted join token or path is never parsed as shell syntax.

# Only needs the mesh-llm binary (no llama.cpp binaries or model).
[positional-arguments]
mesh-client join="" port="9337" console="3131" config="":
    #!/usr/bin/env bash
    set -euo pipefail
    join="$1" port="$2" console="$3" config="$4"
    mesh_bin={{ quote(mesh_bin) }}
    args=(client --port "$port" --console "$console")
    if [[ -n "$join" ]]; then
        args+=(--join "$join")
    fi
    if [[ -n "$config" ]]; then
        args+=(--config "$config")
    fi
    "$mesh_bin" "${args[@]}"

# Build and auto-join a mesh (discover via Nostr)
auto: build
    "{{ mesh_bin }}" --auto
