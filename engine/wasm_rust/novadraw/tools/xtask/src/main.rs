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
    markdown_roots: Vec<String>,
    typed_markdown_roots: Vec<String>,
    command_reference_roots: Vec<String>,
    allowed_document_types: BTreeSet<String>,
    index_sections: Vec<DocumentationIndexSectionSpec>,
    public_api_command: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DocumentationIndexSectionSpec {
    index: String,
    heading: String,
    target_root: String,
    allowed_types: BTreeSet<String>,
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
    NativeWindows,
    NativeLinuxX11,
    NativeLinuxWayland,
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
    #[serde(default)]
    api_semantics: Vec<String>,
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
        let api_semantics = parity_family_ids(root, &self.documentation.parity_ledgers)?;
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
            if matches!(suite.kind, SuiteKind::Contract | SuiteKind::Application)
                && suite.api_semantics.is_empty()
            {
                bail!(
                    "suite `{}` must map at least one parity API family",
                    suite.id
                );
            }
            let mut suite_semantics = HashSet::new();
            for family in &suite.api_semantics {
                if !suite_semantics.insert(family) {
                    bail!(
                        "suite `{}` maps API family `{family}` more than once",
                        suite.id
                    );
                }
                if !api_semantics.contains(family) {
                    bail!(
                        "suite `{}` maps unknown parity API family `{family}`",
                        suite.id
                    );
                }
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
        if self.documentation.allowed_document_types.is_empty() {
            bail!("documentation.allowed_document_types must not be empty");
        }
        if !self
            .commands
            .contains_key(&self.documentation.public_api_command)
        {
            bail!(
                "documentation.public_api_command references unknown command `{}`",
                self.documentation.public_api_command
            );
        }
        validate_documentation(root, self)?;
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

fn parity_family_ids(root: &Path, ledgers: &[String]) -> Result<BTreeSet<String>> {
    let mut families = BTreeSet::new();
    for ledger in ledgers {
        let path = root.join(ledger);
        let source = fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        for line in source.lines().filter(|line| line.starts_with("| `")) {
            let columns = line
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect::<Vec<_>>();
            if columns.len() == 5 {
                families.insert(columns[0].trim_matches('`').to_owned());
            }
        }
    }
    if families.is_empty() {
        bail!("documentation parity ledgers contain no API families");
    }
    Ok(families)
}

fn validate_documentation(root: &Path, manifest: &VerificationManifest) -> Result<()> {
    let documentation = &manifest.documentation;
    let markdown_files = markdown_files_for_roots(root, &documentation.markdown_roots)?;
    for path in &markdown_files {
        validate_markdown_links(root, path)?;
    }

    for path in markdown_files_for_roots(root, &documentation.typed_markdown_roots)? {
        let document_type = read_document_type(&path)?;
        if !documentation
            .allowed_document_types
            .contains(&document_type)
        {
            bail!(
                "{} uses unsupported document type `{document_type}`",
                path.display()
            );
        }
    }

    for path in markdown_files_for_roots(root, &documentation.command_reference_roots)? {
        validate_xtask_references(&path, manifest)?;
    }

    for section in &documentation.index_sections {
        validate_index_section(root, section)?;
    }
    Ok(())
}

fn markdown_files_for_roots(root: &Path, roots: &[String]) -> Result<BTreeSet<PathBuf>> {
    if roots.is_empty() {
        bail!("documentation markdown roots must not be empty");
    }
    let mut files = BTreeSet::new();
    for relative in roots {
        validate_existing_path(root, relative, "documentation root")?;
        collect_markdown_files(&root.join(relative), &mut files)?;
    }
    Ok(files)
}

fn collect_markdown_files(path: &Path, files: &mut BTreeSet<PathBuf>) -> Result<()> {
    if path.is_file() {
        if path.extension().is_some_and(|extension| extension == "md") {
            files.insert(path.to_path_buf());
        }
        return Ok(());
    }
    let entries =
        fs::read_dir(path).with_context(|| format!("failed to read {}", path.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("failed to read {}", path.display()))?;
        collect_markdown_files(&entry.path(), files)?;
    }
    Ok(())
}

fn read_document_type(path: &Path) -> Result<String> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    source
        .lines()
        .find_map(|line| {
            line.strip_prefix("类型：`")
                .and_then(|value| value.strip_suffix('`'))
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
        .with_context(|| format!("{} is missing a `类型：`...`` marker", path.display()))
}

fn validate_markdown_links(root: &Path, path: &Path) -> Result<()> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    for (line_index, line) in source.lines().enumerate() {
        for raw_target in markdown_links(line) {
            let Some(target) = local_markdown_target(raw_target) else {
                continue;
            };
            let resolved = path
                .parent()
                .expect("markdown file has a parent directory")
                .join(target);
            if !resolved.exists() {
                let relative = path.strip_prefix(root).unwrap_or(path);
                bail!(
                    "{}:{} references missing local path `{target}`",
                    relative.display(),
                    line_index + 1
                );
            }
        }
    }
    Ok(())
}

fn markdown_links(line: &str) -> Vec<&str> {
    let mut links = Vec::new();
    let mut remaining = line;
    while let Some(start) = remaining.find("](") {
        let target = &remaining[start + 2..];
        let Some(end) = target.find(')') else {
            break;
        };
        links.push(&target[..end]);
        remaining = &target[end + 1..];
    }
    links
}

fn local_markdown_target(raw: &str) -> Option<&str> {
    let target = raw
        .split_whitespace()
        .next()?
        .trim()
        .trim_start_matches('<')
        .trim_end_matches('>');
    if target.is_empty()
        || target.starts_with('#')
        || target.starts_with("mailto:")
        || target.contains("://")
    {
        return None;
    }
    target
        .split(['#', '?'])
        .next()
        .filter(|target| !target.is_empty())
}

fn validate_xtask_references(path: &Path, manifest: &VerificationManifest) -> Result<()> {
    let source =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    for (line_index, line) in source.lines().enumerate() {
        let mut remaining = line;
        while let Some(start) = remaining.find("cargo xtask ") {
            let reference = &remaining[start + "cargo xtask ".len()..];
            validate_xtask_reference(reference, manifest)
                .with_context(|| format!("{}:{}", path.display(), line_index + 1))?;
            remaining = reference.get(1..).unwrap_or_default();
        }
    }
    Ok(())
}

fn validate_xtask_reference(reference: &str, manifest: &VerificationManifest) -> Result<()> {
    let tokens = reference
        .split_whitespace()
        .map(clean_command_token)
        .filter(|token| !token.is_empty())
        .take(2)
        .collect::<Vec<_>>();
    let Some(command) = tokens.first().copied() else {
        bail!("empty `cargo xtask` reference");
    };
    match command {
        "list" | "docs" => Ok(()),
        "check" => {
            let profile = required_reference_argument(&tokens, command)?
                .strip_prefix("--")
                .unwrap_or(tokens[1]);
            if manifest.profiles.contains_key(profile) {
                Ok(())
            } else {
                bail!("unknown check profile `{profile}`")
            }
        }
        "verify" => {
            let selector = required_reference_argument(&tokens, command)?;
            if selector == "<suite-id>" || selector == "--all" {
                Ok(())
            } else {
                manifest.resolve_suite(selector).map(|_| ())
            }
        }
        "manual" => {
            let selector = required_reference_argument(&tokens, command)?;
            if selector == "<suite-id>" {
                return Ok(());
            }
            let suite = manifest.resolve_suite(selector)?;
            if suite.manual.is_some() {
                Ok(())
            } else {
                bail!("suite `{}` has no manual step", suite.id)
            }
        }
        "run" => {
            let command_id = required_reference_argument(&tokens, command)?;
            if manifest.commands.contains_key(command_id) {
                Ok(())
            } else {
                bail!("unknown command `{command_id}`")
            }
        }
        _ => bail!("unknown xtask subcommand `{command}`"),
    }
}

fn clean_command_token(token: &str) -> &str {
    let token = token.trim_start_matches(['`', '"', '\'']);
    let end = token
        .find([
            '`', '"', '\'', ',', ';', ':', ')', ']', '}', '|', '，', '。', '；', '：', '、', '）',
        ])
        .unwrap_or(token.len());
    &token[..end]
}

fn required_reference_argument<'a>(tokens: &'a [&str], command: &str) -> Result<&'a str> {
    tokens
        .get(1)
        .copied()
        .with_context(|| format!("`cargo xtask {command}` is missing an argument"))
}

fn validate_index_section(root: &Path, spec: &DocumentationIndexSectionSpec) -> Result<()> {
    validate_existing_path(root, &spec.index, "documentation index")?;
    validate_existing_path(root, &spec.target_root, "documentation index target root")?;
    if spec.allowed_types.is_empty() {
        bail!(
            "documentation index section `{}#{}` has no allowed types",
            spec.index,
            spec.heading
        );
    }
    let index_path = root.join(&spec.index);
    let source = fs::read_to_string(&index_path)
        .with_context(|| format!("failed to read {}", index_path.display()))?;
    let marker = format!("## {}", spec.heading);
    let mut in_section = false;
    let mut section_source = String::new();
    for line in source.lines() {
        if line.starts_with("## ") {
            if in_section {
                break;
            }
            in_section = line.trim() == marker;
            continue;
        }
        if in_section {
            section_source.push_str(line);
            section_source.push('\n');
        }
    }
    if !in_section {
        bail!("{} is missing section `{marker}`", spec.index);
    }

    let target_root = fs::canonicalize(root.join(&spec.target_root))
        .with_context(|| format!("failed to resolve {}", spec.target_root))?;
    let mut checked = 0;
    for raw_target in markdown_links(&section_source) {
        let Some(target) = local_markdown_target(raw_target) else {
            continue;
        };
        let target_path = index_path
            .parent()
            .expect("documentation index has a parent directory")
            .join(target);
        if target_path
            .extension()
            .is_none_or(|extension| extension != "md")
            || !target_path.exists()
        {
            continue;
        }
        let canonical = fs::canonicalize(&target_path)
            .with_context(|| format!("failed to resolve {}", target_path.display()))?;
        if !canonical.starts_with(&target_root) {
            continue;
        }
        checked += 1;
        let document_type = read_document_type(&target_path)?;
        validate_indexed_document_type(spec, target, &document_type)?;
    }
    if checked == 0 {
        bail!(
            "{} section `{}` contains no markdown documents under {}",
            spec.index,
            spec.heading,
            spec.target_root
        );
    }
    Ok(())
}

fn validate_indexed_document_type(
    spec: &DocumentationIndexSectionSpec,
    target: &str,
    document_type: &str,
) -> Result<()> {
    if spec.allowed_types.contains(document_type) {
        return Ok(());
    }
    bail!(
        "{} section `{}` classifies {} as `{document_type}`; expected one of {}",
        spec.index,
        spec.heading,
        target,
        spec.allowed_types
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            .join(", ")
    )
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
            manifest.run_command(&root, &manifest.documentation.public_api_command)?;
            println!(
                "PASS {}: {} commands, {} profiles, {} suites; documentation metadata, links, \
                 xtask references, index classification, and public API probes are valid",
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

    #[test]
    fn documentation_commands_distinguish_suites_from_commands() {
        let root = workspace_root().unwrap();
        let manifest = VerificationManifest::load(&root).unwrap();
        assert!(validate_xtask_reference("verify workspace.quality", &manifest).is_ok());
        assert!(validate_xtask_reference("run workspace.test", &manifest).is_ok());
        assert!(validate_xtask_reference("manual g5.4", &manifest).is_ok());
        assert!(validate_xtask_reference("run workspace.quality", &manifest).is_err());
        assert!(validate_xtask_reference("verify missing.suite", &manifest).is_err());
    }

    #[test]
    fn documentation_index_rejects_wrong_document_type() {
        let spec = DocumentationIndexSectionSpec {
            index: "doc/design/00-index.md".to_owned(),
            heading: "核心设计".to_owned(),
            target_root: "doc/design".to_owned(),
            allowed_types: BTreeSet::from(["normative-design".to_owned()]),
        };
        assert!(validate_indexed_document_type(&spec, "valid.md", "normative-design").is_ok());
        assert!(validate_indexed_document_type(&spec, "proposal.md", "proposal").is_err());
    }

    #[test]
    fn markdown_link_parser_keeps_local_targets_and_ignores_anchors() {
        let links = markdown_links("[local](guide.md#section) [web](https://example.com)");
        assert_eq!(local_markdown_target(links[0]), Some("guide.md"));
        assert_eq!(local_markdown_target(links[1]), None);
        assert_eq!(local_markdown_target("#section"), None);
    }
}
