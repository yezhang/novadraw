use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{Context, Result, bail};
use serde::Deserialize;

const MANIFEST_PATH: &str = "verification/suites.toml";
const SUPPORTED_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerificationManifest {
    schema_version: u32,
    evidence_root: String,
    documentation: DocumentationSpec,
    commands: BTreeMap<String, CommandSpec>,
    profiles: BTreeMap<String, ProfileSpec>,
    suites: Vec<SuiteSpec>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentationSpec {
    parity_ledgers: Vec<String>,
    allowed_parity_statuses: BTreeSet<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandSpec {
    description: String,
    program: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    env: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProfileSpec {
    description: String,
    commands: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum SuiteKind {
    Quality,
    Contract,
    Application,
    Build,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum VerificationLayer {
    Static,
    Contract,
    Headless,
    Visual,
    Platform,
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum VerificationPlatform {
    Host,
    Headless,
    NativeMacos,
    Wasm,
    Web,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ManualSpec {
    command: String,
    document: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SuiteSpec {
    id: String,
    title: String,
    milestone: String,
    kind: SuiteKind,
    layers: Vec<VerificationLayer>,
    platforms: Vec<VerificationPlatform>,
    commands: Vec<String>,
    #[serde(default)]
    paths: Vec<String>,
    #[serde(default)]
    documents: Vec<String>,
    #[serde(default)]
    artifacts: Vec<String>,
    #[serde(default)]
    manual: Option<ManualSpec>,
}

impl VerificationManifest {
    fn load(root: &Path) -> Result<Self> {
        let path = root.join(MANIFEST_PATH);
        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        toml::from_str(&source).with_context(|| format!("failed to parse {}", path.display()))
    }

    fn validate(&self, root: &Path) -> Result<()> {
        if self.schema_version != SUPPORTED_SCHEMA_VERSION {
            bail!(
                "unsupported manifest schema {}; expected {}",
                self.schema_version,
                SUPPORTED_SCHEMA_VERSION
            );
        }
        validate_relative_path(&self.evidence_root, "evidence_root")?;
        if self.commands.is_empty() {
            bail!("manifest must define at least one command");
        }
        if self.suites.is_empty() {
            bail!("manifest must define at least one suite");
        }

        let mut referenced_commands = BTreeSet::new();
        for (id, command) in &self.commands {
            validate_id(id, "command")?;
            if command.description.trim().is_empty() {
                bail!("command `{id}` has an empty description");
            }
            if command.program.trim().is_empty() {
                bail!("command `{id}` has an empty program");
            }
            if let Some(cwd) = &command.cwd {
                validate_existing_path(root, cwd, &format!("command `{id}` cwd"))?;
            }
        }

        for required in ["quick", "full"] {
            if !self.profiles.contains_key(required) {
                bail!("required check profile `{required}` is missing");
            }
        }
        for (id, profile) in &self.profiles {
            validate_id(id, "profile")?;
            if profile.description.trim().is_empty() {
                bail!("profile `{id}` has an empty description");
            }
            validate_command_refs(
                &profile.commands,
                &self.commands,
                &mut referenced_commands,
                &format!("profile `{id}`"),
            )?;
        }

        let mut suite_ids = HashSet::new();
        for suite in &self.suites {
            validate_id(&suite.id, "suite")?;
            if !suite_ids.insert(suite.id.as_str()) {
                bail!("duplicate suite id `{}`", suite.id);
            }
            if suite.title.trim().is_empty() || suite.milestone.trim().is_empty() {
                bail!("suite `{}` must define title and milestone", suite.id);
            }
            if suite.layers.is_empty() || suite.platforms.is_empty() {
                bail!("suite `{}` must define layers and platforms", suite.id);
            }
            validate_command_refs(
                &suite.commands,
                &self.commands,
                &mut referenced_commands,
                &format!("suite `{}`", suite.id),
            )?;
            for path in &suite.paths {
                validate_relative_path(path, &format!("suite `{}` path", suite.id))?;
            }
            for document in &suite.documents {
                validate_existing_path(root, document, &format!("suite `{}` document", suite.id))?;
            }
            for artifact in &suite.artifacts {
                validate_relative_path(artifact, &format!("suite `{}` artifact", suite.id))?;
            }
            if let Some(manual) = &suite.manual {
                validate_command_refs(
                    std::slice::from_ref(&manual.command),
                    &self.commands,
                    &mut referenced_commands,
                    &format!("suite `{}` manual", suite.id),
                )?;
                validate_existing_path(
                    root,
                    &manual.document,
                    &format!("suite `{}` manual document", suite.id),
                )?;
            }
        }

        for command in self.commands.keys() {
            if !referenced_commands.contains(command) {
                bail!("command `{command}` is not referenced by a profile or suite");
            }
        }

        if self.documentation.allowed_parity_statuses.is_empty() {
            bail!("documentation.allowed_parity_statuses must not be empty");
        }
        for ledger in &self.documentation.parity_ledgers {
            validate_existing_path(root, ledger, "parity ledger")?;
            validate_parity_statuses(
                &root.join(ledger),
                &self.documentation.allowed_parity_statuses,
            )?;
        }
        Ok(())
    }

    fn resolve_suite(&self, selector: &str) -> Result<&SuiteSpec> {
        if let Some(suite) = self.suites.iter().find(|suite| suite.id == selector) {
            return Ok(suite);
        }
        let matches = self
            .suites
            .iter()
            .filter(|suite| suite.id.starts_with(&format!("{selector}.")))
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [suite] => Ok(*suite),
            [] => bail!("unknown suite `{selector}`; run `cargo xtask list`"),
            _ => bail!(
                "suite selector `{selector}` is ambiguous: {}",
                matches
                    .iter()
                    .map(|suite| suite.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    fn run_command(&self, root: &Path, id: &str) -> Result<()> {
        let spec = self
            .commands
            .get(id)
            .with_context(|| format!("unknown command `{id}`"))?;
        let cwd = spec
            .cwd
            .as_deref()
            .map(|path| root.join(path))
            .unwrap_or_else(|| root.to_path_buf());
        let rendered = std::iter::once(spec.program.as_str())
            .chain(spec.args.iter().map(String::as_str))
            .collect::<Vec<_>>()
            .join(" ");
        println!("\n==> {id}: {}", spec.description);
        println!("    $ {rendered}");
        let status = Command::new(&spec.program)
            .args(&spec.args)
            .envs(&spec.env)
            .current_dir(cwd)
            .status()
            .with_context(|| format!("failed to start command `{id}`"))?;
        if !status.success() {
            bail!("command `{id}` failed with {status}");
        }
        Ok(())
    }

    fn run_commands<'a>(
        &self,
        root: &Path,
        commands: impl IntoIterator<Item = &'a String>,
    ) -> Result<()> {
        for command in commands {
            self.run_command(root, command)?;
        }
        Ok(())
    }
}

fn validate_command_refs(
    command_ids: &[String],
    commands: &BTreeMap<String, CommandSpec>,
    referenced: &mut BTreeSet<String>,
    owner: &str,
) -> Result<()> {
    if command_ids.is_empty() {
        bail!("{owner} must reference at least one command");
    }
    let mut local = HashSet::new();
    for id in command_ids {
        if !commands.contains_key(id) {
            bail!("{owner} references unknown command `{id}`");
        }
        if !local.insert(id) {
            bail!("{owner} references command `{id}` more than once");
        }
        referenced.insert(id.clone());
    }
    Ok(())
}

fn validate_id(id: &str, kind: &str) -> Result<()> {
    if id.is_empty()
        || !id.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
    {
        bail!(
            "{kind} id `{id}` must contain only lowercase ASCII letters, digits, `.`, `_`, or `-`"
        );
    }
    Ok(())
}

fn validate_existing_path(root: &Path, path: &str, owner: &str) -> Result<()> {
    validate_relative_path(path, owner)?;
    if !root.join(path).exists() {
        bail!("{owner} references missing path `{path}`");
    }
    Ok(())
}

fn validate_relative_path(path: &str, owner: &str) -> Result<()> {
    let path = Path::new(path);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        bail!("{owner} must be a non-empty workspace-relative path");
    }
    Ok(())
}

fn validate_parity_statuses(path: &Path, allowed: &BTreeSet<String>) -> Result<()> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let mut checked_rows = 0;
    for (index, line) in source.lines().enumerate() {
        if !line.starts_with("| `") {
            continue;
        }
        let columns = line
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        if columns.len() != 5 {
            continue;
        }
        checked_rows += 1;
        let status = columns[3].trim_matches('`');
        if !allowed.contains(status) {
            bail!(
                "{}:{} uses unsupported parity status `{status}`",
                path.display(),
                index + 1
            );
        }
    }
    if checked_rows == 0 {
        bail!("{} contains no parity status rows", path.display());
    }
    Ok(())
}

fn workspace_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .context("xtask must live at tools/xtask")
}

fn print_help() {
    println!(
        "Novadraw workflow\n\n\
         Usage:\n\
           cargo xtask list\n\
           cargo xtask docs\n\
           cargo xtask check --quick|--full\n\
           cargo xtask verify <suite-id|--all>\n\
           cargo xtask manual <suite-id>\n\
           cargo xtask run <command-id>"
    );
}

fn run() -> Result<()> {
    let root = workspace_root()?;
    let manifest = VerificationManifest::load(&root)?;
    manifest.validate(&root)?;
    let args = env::args().skip(1).collect::<Vec<_>>();

    match args.as_slice() {
        [command] if command == "list" => {
            println!("Profiles:");
            for (id, profile) in &manifest.profiles {
                println!("  {id:<8} {}", profile.description);
            }
            println!("\nSuites:");
            for suite in &manifest.suites {
                let manual = if suite.manual.is_some() {
                    " + manual"
                } else {
                    ""
                };
                println!(
                    "  {:<32} {:<9} {}{}",
                    suite.id,
                    format!("{:?}", suite.kind).to_lowercase(),
                    suite.title,
                    manual
                );
            }
        }
        [command] if command == "docs" => {
            println!(
                "PASS {}: {} commands, {} profiles, {} suites",
                MANIFEST_PATH,
                manifest.commands.len(),
                manifest.profiles.len(),
                manifest.suites.len()
            );
        }
        [command, profile] if command == "check" => {
            let profile = profile.strip_prefix("--").unwrap_or(profile);
            let profile = manifest
                .profiles
                .get(profile)
                .with_context(|| format!("unknown check profile `{profile}`"))?;
            manifest.run_commands(&root, &profile.commands)?;
        }
        [command, selector] if command == "verify" && selector == "--all" => {
            let mut seen = HashSet::new();
            let mut commands = Vec::new();
            for suite in &manifest.suites {
                for command in &suite.commands {
                    if seen.insert(command.as_str()) {
                        commands.push(command);
                    }
                }
            }
            manifest.run_commands(&root, commands)?;
        }
        [command, selector] if command == "verify" => {
            let suite = manifest.resolve_suite(selector)?;
            println!("Suite {} ({})", suite.id, suite.milestone);
            manifest.run_commands(&root, &suite.commands)?;
        }
        [command, selector] if command == "manual" => {
            let suite = manifest.resolve_suite(selector)?;
            let manual = suite
                .manual
                .as_ref()
                .with_context(|| format!("suite `{}` has no manual step", suite.id))?;
            println!("Manual guide: {}", root.join(&manual.document).display());
            manifest.run_command(&root, &manual.command)?;
        }
        [command, command_id] if command == "run" => {
            manifest.run_command(&root, command_id)?;
        }
        [] | [_]
            if args
                .first()
                .is_none_or(|arg| arg == "help" || arg == "--help") =>
        {
            print_help();
        }
        _ => {
            print_help();
            bail!("invalid arguments");
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("xtask: {error:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repository_manifest_is_valid() {
        let root = workspace_root().unwrap();
        let manifest = VerificationManifest::load(&root).unwrap();
        manifest.validate(&root).unwrap();
    }

    #[test]
    fn suite_prefix_must_be_unique() {
        let root = workspace_root().unwrap();
        let manifest = VerificationManifest::load(&root).unwrap();
        assert!(manifest.resolve_suite("g5").is_err());
        assert_eq!(
            manifest.resolve_suite("g5.4").unwrap().id,
            "g5.4.connection-bendpoint"
        );
    }
}
