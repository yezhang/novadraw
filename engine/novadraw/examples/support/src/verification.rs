use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;

pub type VerificationMetrics = BTreeMap<String, String>;
pub type VerificationFn = fn() -> Result<VerificationMetrics, String>;

#[derive(Clone, Copy)]
pub struct VerificationCase {
    pub name: &'static str,
    pub run: VerificationFn,
}

#[derive(Clone, Debug, Default)]
pub struct VerificationCli {
    pub verify: bool,
    pub scenario: Option<String>,
    pub screenshot: Option<String>,
    pub screenshot_all: bool,
    pub report: Option<PathBuf>,
}

impl VerificationCli {
    pub fn parse() -> Result<Self, String> {
        let mut cli = Self::default();
        for argument in std::env::args().skip(1) {
            if argument == "--verify" {
                cli.verify = true;
            } else if argument == "--screenshot-all" {
                cli.screenshot_all = true;
            } else if let Some(value) = argument.strip_prefix("--scenario=") {
                cli.scenario = Some(value.to_string());
            } else if let Some(value) = argument.strip_prefix("--screenshot=") {
                cli.screenshot = Some(value.to_string());
            } else if let Some(value) = argument.strip_prefix("--report=") {
                cli.report = Some(PathBuf::from(value));
            } else if argument == "--help" || argument == "-h" {
                return Err("--verify [--scenario=<name>] [--report=<path>] | \
                     --screenshot=<name|index> | --screenshot-all"
                    .to_string());
            } else {
                return Err(format!("unknown argument: {argument}"));
            }
        }
        Ok(cli)
    }
}

#[derive(Serialize)]
struct VerificationReport {
    app: String,
    generated_at_unix_seconds: u64,
    passed: bool,
    cases: Vec<CaseReport>,
}

#[derive(Serialize)]
struct CaseReport {
    name: String,
    passed: bool,
    metrics: VerificationMetrics,
    error: Option<String>,
}

pub fn run_verification(
    app: &str,
    cases: &[VerificationCase],
    scenario: Option<&str>,
    report_path: Option<&Path>,
) -> Result<PathBuf, String> {
    let selected = cases
        .iter()
        .filter(|case| scenario.is_none_or(|name| case.name == name))
        .collect::<Vec<_>>();
    if selected.is_empty() {
        return Err(format!("unknown verification scenario: {:?}", scenario));
    }

    let mut reports = Vec::with_capacity(selected.len());
    for case in selected {
        match (case.run)() {
            Ok(metrics) => {
                println!("PASS {}", case.name);
                reports.push(CaseReport {
                    name: case.name.to_string(),
                    passed: true,
                    metrics,
                    error: None,
                });
            }
            Err(error) => {
                eprintln!("FAIL {}: {}", case.name, error);
                reports.push(CaseReport {
                    name: case.name.to_string(),
                    passed: false,
                    metrics: VerificationMetrics::new(),
                    error: Some(error),
                });
            }
        }
    }

    let passed = reports.iter().all(|report| report.passed);
    let report = VerificationReport {
        app: app.to_string(),
        generated_at_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        passed,
        cases: reports,
    };
    let output = report_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("target/visual-verification").join(format!("{app}.json")));
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let json = serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?;
    std::fs::write(&output, json).map_err(|error| error.to_string())?;
    println!("REPORT {}", output.display());

    if passed {
        Ok(output)
    } else {
        Err(format!("{app} verification failed"))
    }
}
