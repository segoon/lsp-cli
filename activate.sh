# Source this from the repo root to put the tools installed by `make download-dev-env`
# (go, java, node, dotnet) on PATH: `source activate.sh`
case ":$PATH:" in
    *":$PWD/.env/bin:"*) ;;
    *) export PATH="$PWD/.env/bin:$PATH" ;;
esac
