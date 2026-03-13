use std::fs;

use serde_json::Value;
use zed_extension_api::{
    self as zed, serde_json, DebugAdapterBinary, DebugConfig, DebugRequest, DebugScenario,
    DebugTaskDefinition, Result, StartDebuggingRequestArguments,
    StartDebuggingRequestArgumentsRequest,
};

const GITHUB_REPO: &str = "Samsung/netcoredbg";

struct NetCoreDbgExtension {
    cached_binary_path: Option<String>,
}

impl NetCoreDbgExtension {
    /// Find or download the netcoredbg binary.
    fn ensure_binary(
        &mut self,
        user_provided_path: Option<String>,
        worktree: &zed::Worktree,
    ) -> Result<String> {
        // 1. User-provided path takes priority
        if let Some(path) = user_provided_path {
            return Ok(path);
        }

        // 2. Check if netcoredbg is on PATH
        if let Some(path) = worktree.which("netcoredbg") {
            return Ok(path);
        }

        // 3. Check cached path from a previous download
        if let Some(ref path) = self.cached_binary_path {
            if is_file(path) {
                return Ok(path.clone());
            }
        }

        // 4. Download from Samsung/netcoredbg GitHub releases
        let release = zed::latest_github_release(
            GITHUB_REPO,
            zed::GithubReleaseOptions {
                require_assets: true,
                pre_release: false,
            },
        )?;

        let (os, arch) = zed::current_platform();

        let (asset_name, file_type) = match (os, arch) {
            (zed::Os::Linux, zed::Architecture::X8664) => (
                "netcoredbg-linux-amd64.tar.gz",
                zed::DownloadedFileType::GzipTar,
            ),
            (zed::Os::Linux, zed::Architecture::Aarch64) => (
                "netcoredbg-linux-arm64.tar.gz",
                zed::DownloadedFileType::GzipTar,
            ),
            // Samsung only publishes x64 macOS builds; ARM64 Macs run them via Rosetta 2
            (zed::Os::Mac, zed::Architecture::X8664 | zed::Architecture::Aarch64) => (
                "netcoredbg-osx-amd64.tar.gz",
                zed::DownloadedFileType::GzipTar,
            ),
            (zed::Os::Windows, zed::Architecture::X8664) => (
                "netcoredbg-win64.zip",
                zed::DownloadedFileType::Zip,
            ),
            _ => return Err(format!("unsupported platform: {os:?}/{arch:?}")),
        };

        let asset = release
            .assets
            .iter()
            .find(|a| a.name == asset_name)
            .ok_or_else(|| format!("no netcoredbg asset found: {asset_name}"))?;

        let version_dir = format!("netcoredbg-{}", release.version);
        let exe_name = if matches!(os, zed::Os::Windows) {
            "netcoredbg.exe"
        } else {
            "netcoredbg"
        };

        // Samsung archives extract into a `netcoredbg/` subdirectory
        let direct_path = format!("{version_dir}/{exe_name}");
        let nested_path = format!("{version_dir}/netcoredbg/{exe_name}");

        if !is_file(&direct_path) && !is_file(&nested_path) {
            // Clean up old version directories
            if let Ok(entries) = fs::read_dir(".") {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if name.starts_with("netcoredbg-") && name.as_ref() != version_dir {
                        fs::remove_dir_all(entry.path()).ok();
                    }
                }
            }

            zed::download_file(&asset.download_url, &version_dir, file_type)
                .map_err(|e| format!("failed to download netcoredbg: {e}"))?;

            // Make executable (needed for Linux/macOS, no-op on Windows)
            zed::make_file_executable(&direct_path).ok();
            zed::make_file_executable(&nested_path).ok();
        }

        // Locate the binary — try direct, nested, then recursive search
        if is_file(&direct_path) {
            self.cached_binary_path = Some(direct_path.clone());
            Ok(direct_path)
        } else if is_file(&nested_path) {
            self.cached_binary_path = Some(nested_path.clone());
            Ok(nested_path)
        } else if let Some(found) = find_binary_recursive(&version_dir, exe_name) {
            zed::make_file_executable(&found).ok();
            self.cached_binary_path = Some(found.clone());
            Ok(found)
        } else {
            Err("netcoredbg binary not found after download. Please report this issue at https://github.com/NeverGET/netcoredbg-zed/issues".into())
        }
    }
}

/// Check if a path points to an existing file.
fn is_file(path: &str) -> bool {
    fs::metadata(path).map(|m| m.is_file()).unwrap_or(false)
}

/// Search one level of subdirectories for a binary by name.
fn find_binary_recursive(dir: &str, binary_name: &str) -> Option<String> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            let candidate = format!("{}/{}", entry.path().display(), binary_name);
            if is_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

impl zed::Extension for NetCoreDbgExtension {
    fn new() -> Self {
        Self {
            cached_binary_path: None,
        }
    }

    fn get_dap_binary(
        &mut self,
        _adapter_name: String,
        config: DebugTaskDefinition,
        user_provided_debug_adapter_path: Option<String>,
        worktree: &zed::Worktree,
    ) -> Result<DebugAdapterBinary, String> {
        let binary_path = self.ensure_binary(user_provided_debug_adapter_path, worktree)?;

        let parsed: Value = serde_json::from_str(&config.config)
            .unwrap_or(Value::Object(serde_json::Map::new()));

        let request = parsed
            .get("request")
            .and_then(|r| r.as_str())
            .unwrap_or("launch");

        let request_kind = match request {
            "attach" => StartDebuggingRequestArgumentsRequest::Attach,
            _ => StartDebuggingRequestArgumentsRequest::Launch,
        };

        Ok(DebugAdapterBinary {
            command: Some(binary_path),
            arguments: vec!["--interpreter=vscode".to_string()],
            envs: vec![],
            cwd: None,
            connection: None,
            request_args: StartDebuggingRequestArguments {
                configuration: config.config,
                request: request_kind,
            },
        })
    }

    fn dap_request_kind(
        &mut self,
        _adapter_name: String,
        config: Value,
    ) -> Result<StartDebuggingRequestArgumentsRequest, String> {
        let request = config
            .get("request")
            .and_then(|r| r.as_str())
            .unwrap_or("launch");

        match request {
            "launch" => Ok(StartDebuggingRequestArgumentsRequest::Launch),
            "attach" => Ok(StartDebuggingRequestArgumentsRequest::Attach),
            _ => Err(format!("Unknown request type: {}", request)),
        }
    }

    fn dap_config_to_scenario(&mut self, config: DebugConfig) -> Result<DebugScenario, String> {
        let mut dap_config = serde_json::Map::new();

        dap_config.insert(
            "type".to_string(),
            Value::String("coreclr".to_string()),
        );
        dap_config.insert(
            "name".to_string(),
            Value::String(config.label.clone()),
        );

        let request_str = match &config.request {
            DebugRequest::Launch(launch) => {
                dap_config.insert(
                    "program".to_string(),
                    Value::String(launch.program.clone()),
                );
                if let Some(cwd) = &launch.cwd {
                    dap_config.insert("cwd".to_string(), Value::String(cwd.clone()));
                }
                if !launch.args.is_empty() {
                    dap_config.insert(
                        "args".to_string(),
                        Value::Array(
                            launch.args.iter().map(|a| Value::String(a.clone())).collect(),
                        ),
                    );
                }
                if !launch.envs.is_empty() {
                    let env_map: serde_json::Map<String, Value> = launch
                        .envs
                        .iter()
                        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                        .collect();
                    dap_config.insert("env".to_string(), Value::Object(env_map));
                }
                "launch"
            }
            DebugRequest::Attach(attach) => {
                if let Some(pid) = attach.process_id {
                    dap_config.insert(
                        "processId".to_string(),
                        Value::Number(serde_json::Number::from(pid)),
                    );
                }
                "attach"
            }
        };

        dap_config.insert(
            "request".to_string(),
            Value::String(request_str.to_string()),
        );

        if let Some(stop) = config.stop_on_entry {
            dap_config.insert("stopAtEntry".to_string(), Value::Bool(stop));
        }

        Ok(DebugScenario {
            label: config.label,
            adapter: "netcoredbg".to_string(),
            config: serde_json::to_string(&Value::Object(dap_config)).unwrap(),
            tcp_connection: None,
            build: None,
        })
    }
}

zed::register_extension!(NetCoreDbgExtension);
