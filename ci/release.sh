#!/bin/bash

set -euo pipefail

set_version () {
    # Version specifications in Cargo.toml/lock
    sed -i '/transmission-proxy\|transmission-rpc-client/{n;s/^version = "[^"]*"/version = "'"$1"'"/}' crates/*/Cargo.toml Cargo.lock

    # Dependency to transmission-rpc-client
    sed -i '/transmission-rpc-client\s*=/{s/version = "[^"]*"/version = "'"$1"'"/}' crates/transmission-proxy/Cargo.toml Cargo.lock
    sed -i '/transmission-rpc-client\s*=/{s/= "[^"]*"/= "'"$1"'"/}' crates/transmission-proxy/Cargo.toml
}

action="$1"
shift

case "$action" in
    set-version)
        set_version "$@"
        ;;
    *)
        echo "Usage: ./ci/release.sh set-version|publish"
        ;;
esac
