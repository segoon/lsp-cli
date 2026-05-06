# How `--download` works

`--download` lets `lsp-cli` install a missing language server automatically and then use that installed copy for the current command. This changes `lsp-cli` from "use only what is already on this machine" to "fetch what is needed for this language server when possible".

This behavior is not limited to commands that later talk to a server. `detect --download` may also install a server, because allowing downloads can change which server becomes runnable and therefore which command `lsp-cli` suggests.

## What Gets Installed

`lsp-cli` installs the selected language server for the current user. It does not install a full editor or IDE. Depending on the server, the installed result may be a ready-to-run executable, a package installed through an existing package tool on your machine, or a downloaded archive unpacked into `lsp-cli`'s own storage area. Some servers also need extra support files, and those are stored together with the installed server.

If a matching server is already available on your system, `lsp-cli` may use that existing copy instead of installing a new one.

## Where Files Are Stored

Downloaded servers are stored in the current user's `lsp-cli` data area under `~/.local/share/lsp-cli/`. In practice, installed server files are kept under `~/.local/share/lsp-cli/packages/`. `lsp-cli` may also create helper launchers under `~/.local/share/lsp-cli/bin/`, store shared support files under `~/.local/share/lsp-cli/share/`, keep installation records under `~/.local/share/lsp-cli/receipts/`, and cache server registry data under `~/.local/share/lsp-cli/registry/`.

This means the download does not go into the project directory and does not require a system-wide install. The installed server is reused by later `lsp-cli` runs for the same user.

## How Downloaded Servers Start

After installation, `lsp-cli` starts the selected server itself. In some cases it runs the downloaded executable directly. In other cases it starts a small helper launcher created by `lsp-cli`, and that launcher uses another program already present on the machine, such as `python3`, `node`, `dotnet`, or `java`, to run the installed server.

On Unix systems, some helper launchers also rely on `/bin/sh`.

## External Requirements

The first use of `--download` needs network access. After that, later runs can usually reuse the previously installed copy.

The exact external requirement depends on the selected server. Some servers can be downloaded and run directly. Others require an existing tool already available in `PATH`, so that `lsp-cli` can install them. Some installed servers also need an existing runtime to launch them.

Possible required executables in `PATH`:
- `npm`
- `python3`
- `cargo`
- `go`
- `node`
- `dotnet`
- `java`

`lsp-cli` also needs a normal per-user home directory, because its download state is stored there.

## Security Risks

Using `--download` means allowing `lsp-cli` to fetch software from external sources and then run that software with your user account. This is a larger trust decision than using only servers you installed yourself beforehand.

The downloaded files are kept inside `lsp-cli`'s per-user storage area rather than mixed into your project, and `lsp-cli` rejects some obviously unsafe archive layouts. Even so, automatic installation is not risk-free. A compromised upstream package, a malicious release, or a server with its own security problems can still affect your machine or your code.

If your environment has strict security requirements, the safer choice is usually to install and review allowed language servers separately and run `lsp-cli` without `--download`.

## How It May Fail

The main reasons why `--download` can fail:
- no suitable language server is known for the detected language
- automatic installation is not supported for the selected server
- your operating system or CPU architecture is not supported by the available download
- required external tools are missing from `PATH`
- the network is unavailable
- the remote download or registry service is temporarily failing
- the downloaded archive cannot be unpacked safely
- the installation completes but does not produce a runnable server

It can also fail when the local environment is incomplete, for example if the home directory cannot be resolved. In some cases `lsp-cli` may still continue if one candidate server fails but another matching server works.
