# foac

foac, the Father Of All CLIs

## Install

[ubi](https://github.com/houseabsolute/ubi) installs the `foac` binary for your platform from [GitHub Releases](https://github.com/lra/foac/releases).

### 1. Install ubi

On Linux, macOS, FreeBSD, and NetBSD:

```sh
curl --silent --location \
    https://raw.githubusercontent.com/houseabsolute/ubi/master/bootstrap/bootstrap-ubi.sh |
    sh
```

This command installs `ubi` into `$HOME/bin` for a normal user, or into `/usr/local/bin` for root.

On Windows, run this command in PowerShell. It installs `ubi.exe` into the current directory:

```powershell
powershell -exec bypass -c "Invoke-WebRequest -URI 'https://raw.githubusercontent.com/houseabsolute/ubi/master/bootstrap/bootstrap-ubi.ps1' -UseBasicParsing | Invoke-Expression"
```

### 2. Install foac

```sh
ubi --project lra/foac --in "$HOME/bin"
```

On Windows:

```powershell
.\ubi.exe --project lra/foac --in "$HOME\bin"
```

If `$HOME/bin` is not on your `PATH`, add it.

## Run

```sh
foac
foac --help
foac --version
foac update
```

`foac update` downloads the latest GitHub release for this platform and replaces the running binary.

## From source

```sh
cargo run -- --help
```
