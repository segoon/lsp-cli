# Source this from the repo root to put the tools installed by `make download-dev-env`
# (go, java, node, dotnet, zig, ruby) on PATH: `source activate.sh`
case ":$PATH:" in
    *":$PWD/.env/bin:"*) ;;
    *) export PATH="$PWD/.env/bin:$PATH" ;;
esac

# ruby-builder's prebuilt ruby needs its lib dir on LD_LIBRARY_PATH (RUNPATH points at the
# hosted-toolcache path it was built for) and RUBYLIB (default $LOAD_PATH is baked in too).
if [[ -d "$PWD/.env/ruby/lib" ]]; then
    export LD_LIBRARY_PATH="$PWD/.env/ruby/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
    rubylib_entries=()
    for dir in "$PWD"/.env/ruby/lib/ruby/*/; do
        [[ -d "$dir" ]] || continue
        rubylib_entries+=("${dir%/}")
        for arch_dir in "$dir"*linux*/; do
            [[ -d "$arch_dir" ]] && rubylib_entries+=("${arch_dir%/}")
        done
    done
    if (( ${#rubylib_entries[@]} > 0 )); then
        rubylib_joined=$(IFS=:; echo "${rubylib_entries[*]}")
        export RUBYLIB="$rubylib_joined${RUBYLIB:+:$RUBYLIB}"
    fi
    unset rubylib_entries rubylib_joined dir arch_dir
fi
