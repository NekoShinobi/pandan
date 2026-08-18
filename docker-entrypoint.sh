#!/bin/sh
set -eu

# Named cache volumes arrive root-owned. The dev data directory is part of the
# repository bind mount and is created here before privileges are dropped.
for path in ${DEV_CHOWN_PATHS:-}; do
    mkdir -p "$path" 2>/dev/null || true
    [ -e "$path" ] || continue

    if [ "$(stat -c '%u' "$path")" != "${DEV_UID}" ]; then
        chown -R "${DEV_UID}:${DEV_GID}" "$path" 2>/dev/null || true
    fi
done

# Cargo does not include the host libc version in its artifact fingerprints.
# A named target volume can therefore retain build scripts from an older image
# and try to execute binaries linked against a newer glibc. Stamp the cache with
# the complete Rust host description and libc version, and rebuild it whenever
# either changes.
target_dir=/app/target
cache_stamp="$target_dir/.pandan-build-environment"

if [ -d "$target_dir" ]; then
    build_environment="$(
        rustc --version --verbose
        ldd --version 2>&1 | sed -n '1p'
    )"
    reset_cache=false

    if [ -f "$cache_stamp" ]; then
        if [ "$(sed -n '1,$p' "$cache_stamp")" != "$build_environment" ]; then
            reset_cache=true
        fi
    elif [ -n "$(find "$target_dir" -mindepth 1 -maxdepth 1 -print -quit)" ]; then
        reset_cache=true
    fi

    if [ "$reset_cache" = true ]; then
        echo "Rust build environment changed; clearing incompatible target cache" >&2
        find "$target_dir" -mindepth 1 -maxdepth 1 -exec rm -rf -- {} +
    fi

    printf '%s\n' "$build_environment" >"$cache_stamp"
    chown "${DEV_UID}:${DEV_GID}" "$cache_stamp"
fi

exec gosu "${DEV_UID}:${DEV_GID}" "$@"
