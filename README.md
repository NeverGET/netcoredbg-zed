# NetCoreDbg for Zed

.NET Core debugging support for [Zed](https://zed.dev) via Samsung's [netcoredbg](https://github.com/Samsung/netcoredbg) DAP debugger.

## Install

Search for **NetCoreDbg** in Zed's extension marketplace (`zed: extensions`).

The extension automatically downloads the correct netcoredbg binary for your platform on first use.

## Platform Support

| Platform | Status |
|----------|--------|
| Linux x64 | Full support |
| Linux ARM64 | Full support |
| macOS x64 | Full support |
| macOS ARM64 | Via Rosetta 2 |
| Windows x64 | Full support |
| WSL2 (remote) | Automatic (runs as Linux) |

## Configuration

Create `.zed/debug.json` in your project root:

### Launch (run and debug)

```json
[
  {
    "adapter": "netcoredbg",
    "label": "Launch MyApp",
    "request": "launch",
    "program": "${workspaceFolder}/bin/Debug/net8.0/MyApp.dll",
    "cwd": "${workspaceFolder}",
    "args": [],
    "env": {},
    "stopAtEntry": false
  }
]
```

### Attach (debug running process)

```json
[
  {
    "adapter": "netcoredbg",
    "label": "Attach to process",
    "request": "attach",
    "processId": 12345
  }
]
```

## Manual Binary Path

If you prefer to use a specific netcoredbg installation, configure in Zed settings:

```json
{
  "dap": {
    "netcoredbg": {
      "binary": "/path/to/netcoredbg"
    }
  }
}
```

## Prerequisites

- [.NET SDK](https://dotnet.microsoft.com/download) (6.0 or later)
- Your project must be built before debugging (`dotnet build`)

## Credits

- [Samsung/netcoredbg](https://github.com/Samsung/netcoredbg) — the underlying debugger
- [Zed](https://zed.dev) — the editor

## License

MIT
