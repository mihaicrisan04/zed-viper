use std::fs;
use zed_extension_api::{
    self as zed, Architecture, DownloadedFileType, GithubReleaseOptions, LanguageServerId,
    LanguageServerInstallationStatus, Os, Result, Worktree, serde_json::Value, settings::LspSettings,
};

const BRIDGE_REPO: &str = "mihaicrisan04/zed-viper";
const VIPER_TOOLS_REPO: &str = "viperproject/viper-ide";
const BRIDGE_DIR_PREFIX: &str = "viper-lsp-";
const VIPER_TOOLS_DIR_PREFIX: &str = "vipertools-";

const JAVA_MISSING: &str = "Viper needs Java 11 or newer. Install it so `java` is on PATH, \
    or set `lsp.viper-lsp.settings.java_path` in your Zed settings.";

/// `lsp.viper-lsp.settings` in the user's Zed settings.
#[derive(Default)]
struct UserSettings {
    backend: Option<String>,
    java_path: Option<String>,
    viper_tools_path: Option<String>,
}

impl UserSettings {
    fn from_json(settings: Option<Value>) -> Self {
        let field = |name: &str| {
            settings.as_ref().and_then(|s| s.get(name)).and_then(Value::as_str).map(String::from)
        };
        UserSettings {
            backend: field("backend"),
            java_path: field("java_path"),
            viper_tools_path: field("viper_tools_path"),
        }
    }
}

#[derive(Default)]
struct ViperExtension {
    bridge_path: Option<String>,
    viper_tools_path: Option<String>,
}

impl zed::Extension for ViperExtension {
    fn new() -> Self {
        Self::default()
    }

    fn language_server_command(&mut self, id: &LanguageServerId, worktree: &Worktree) -> Result<zed::Command> {
        let lsp_settings = LspSettings::for_worktree(id.as_ref(), worktree).unwrap_or_default();
        let settings = UserSettings::from_json(lsp_settings.settings);
        let (binary_path, binary_arguments, binary_env) = match lsp_settings.binary {
            Some(binary) => (binary.path, binary.arguments, binary.env),
            None => (None, None, None),
        };
        let mut env = worktree.shell_env();
        let env_var = |name: &str| env.iter().find(|(key, _)| key == name).map(|(_, value)| value.clone());

        let java = settings.java_path.or_else(|| worktree.which("java")).ok_or(JAVA_MISSING)?;
        let viper_tools = match settings.viper_tools_path.or_else(|| env_var("VIPER_HOME")) {
            Some(path) => path,
            None => self.viper_tools(id)?,
        };
        let bridge = match binary_path.or_else(|| worktree.which("viper-lsp")) {
            Some(path) => path,
            None => self.bridge(id)?,
        };

        env.push(("VIPER_JAVA".into(), java));
        env.push(("VIPER_HOME".into(), viper_tools));
        if let Some(backend) = settings.backend {
            env.push(("VIPER_BACKEND".into(), backend));
        }
        env.extend(binary_env.unwrap_or_default());

        Ok(zed::Command { command: bridge, args: binary_arguments.unwrap_or_default(), env })
    }
}

impl ViperExtension {
    /// The `viper-lsp` bridge from this repo's GitHub releases.
    fn bridge(&mut self, id: &LanguageServerId) -> Result<String> {
        if let Some(path) = self.bridge_path.as_ref().filter(|path| is_file(path)) {
            return Ok(path.clone());
        }
        let (os, arch) = zed::current_platform();
        let target = match (os, arch) {
            (Os::Mac, Architecture::Aarch64) => "aarch64-apple-darwin",
            (Os::Mac, Architecture::X8664) => "x86_64-apple-darwin",
            (Os::Linux, Architecture::Aarch64) => "aarch64-unknown-linux-gnu",
            (Os::Linux, Architecture::X8664) => "x86_64-unknown-linux-gnu",
            (Os::Windows, Architecture::X8664) => "x86_64-pc-windows-msvc",
            _ => return Err(format!("no viper-lsp build for {os:?} {arch:?}, install it with `cargo install` and put it on PATH")),
        };
        let (archive_type, extension) = match os {
            Os::Windows => (DownloadedFileType::Zip, "zip"),
            _ => (DownloadedFileType::GzipTar, "tar.gz"),
        };
        let executable = if os == Os::Windows { "viper-lsp.exe" } else { "viper-lsp" };

        let path = install_from_release(id, BRIDGE_REPO, BRIDGE_DIR_PREFIX, &format!("viper-lsp-{target}.{extension}"), archive_type, executable, |dir| {
            zed::make_file_executable(&format!("{dir}/{executable}"))
        })?;
        self.bridge_path = Some(path.clone());
        Ok(path)
    }

    /// ViperTools (ViperServer, Z3, Boogie) from the Viper IDE's GitHub releases, about 120 MB.
    fn viper_tools(&mut self, id: &LanguageServerId) -> Result<String> {
        if let Some(path) = self.viper_tools_path.as_ref().filter(|path| is_file(&format!("{path}/backends/viperserver.jar"))) {
            return Ok(path.clone());
        }
        let (os, arch) = zed::current_platform();
        let asset = match (os, arch) {
            (Os::Mac, Architecture::Aarch64) => "ViperToolsMacARM.zip",
            (Os::Mac, Architecture::X8664) => "ViperToolsMac.zip",
            (Os::Linux, Architecture::X8664) => "ViperToolsLinux.zip",
            (Os::Windows, Architecture::X8664) => "ViperToolsWin.zip",
            _ => return Err(format!(
                "ViperTools has no {os:?} {arch:?} build, install Viper yourself and set `lsp.viper-lsp.settings.viper_tools_path`"
            )),
        };
        let relative = install_from_release(id, VIPER_TOOLS_REPO, VIPER_TOOLS_DIR_PREFIX, asset, DownloadedFileType::Zip, "backends/viperserver.jar", |dir| {
            if os == Os::Windows {
                return Ok(());
            }
            zed::make_file_executable(&format!("{dir}/z3/bin/z3"))?;
            zed::make_file_executable(&format!("{dir}/boogie/Binaries/Boogie"))
        })?;
        // ViperServer gets this through an env var, so unlike the bridge command it has to be absolute
        let dir = relative.trim_end_matches("/backends/viperserver.jar");
        let absolute = std::env::current_dir().map_err(|error| error.to_string())?.join(dir);
        let path = absolute.to_string_lossy().into_owned();
        self.viper_tools_path = Some(path.clone());
        Ok(path)
    }
}

/// Downloads `asset` from the latest release of `repo` into `<dir_prefix><version>/` unless that
/// version is already there, and returns the path of `marker` inside it. Falls back to any
/// previously downloaded version when GitHub can't be reached.
fn install_from_release(
    id: &LanguageServerId,
    repo: &str,
    dir_prefix: &str,
    asset: &str,
    archive_type: DownloadedFileType,
    marker: &str,
    after_download: impl FnOnce(&str) -> Result<()>,
) -> Result<String> {
    zed::set_language_server_installation_status(id, &LanguageServerInstallationStatus::CheckingForUpdate);
    let options = GithubReleaseOptions { require_assets: true, pre_release: false };
    let release = match zed::latest_github_release(repo, options) {
        Ok(release) => release,
        Err(error) => {
            return installed_dirs(dir_prefix)
                .into_iter()
                .map(|dir| format!("{dir}/{marker}"))
                .find(|path| is_file(path))
                .ok_or_else(|| format!("could not check {repo} releases: {error}"));
        }
    };

    let dir = format!("{dir_prefix}{}", release.version);
    let path = format!("{dir}/{marker}");
    if !is_file(&path) {
        let url = &release
            .assets
            .iter()
            .find(|candidate| candidate.name == asset)
            .ok_or_else(|| format!("{repo} release {} has no {asset}", release.version))?
            .download_url;
        zed::set_language_server_installation_status(id, &LanguageServerInstallationStatus::Downloading);
        zed::download_file(url, &dir, archive_type).map_err(|error| format!("downloading {asset} failed: {error}"))?;
        after_download(&dir)?;
        for old in installed_dirs(dir_prefix).into_iter().filter(|old| *old != dir) {
            fs::remove_dir_all(old).ok();
        }
    }
    zed::set_language_server_installation_status(id, &LanguageServerInstallationStatus::None);
    Ok(path)
}

fn installed_dirs(prefix: &str) -> Vec<String> {
    let Ok(entries) = fs::read_dir(".") else { return Vec::new() };
    entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.starts_with(prefix))
        .collect()
}

fn is_file(path: &str) -> bool {
    fs::metadata(path).is_ok_and(|metadata| metadata.is_file())
}

zed::register_extension!(ViperExtension);
