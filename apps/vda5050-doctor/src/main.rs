use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand, ValueEnum};
use serde::Serialize;
use vda5050_core::{
    Assertion, AssertionTrust, Inference, InferenceConfidence, Observation, Recommendation,
    ReportValidity,
};
use vda5050_doctor::{load_synthetic_evidence_manifest, trace_context_with_synthetic_evidence};
use vda5050_doctor_engine::{DiagnosticReport, Finding, analyze};
use vda5050_import::{
    ImportConfig, ImportFormat, ImportReport, RecordKind, RejectReason, import_path,
};
use vda5050_report::{
    EvidenceReference, IncidentReport, RenderLimits, SpecificationReference, render_terminal,
    to_canonical_json_value,
};

const PROTOCOL_BUNDLE_ID: &str = "vda5050-3.0.0-phase1@2026-08-06";

#[derive(Debug, Parser)]
#[command(name = "vda5050-doctor")]
#[command(about = "Offline, evidence-bounded diagnosis for VDA 5050 traces")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Diagnose {
        input: PathBuf,
        #[arg(long)]
        vda_version: String,
        #[arg(long, value_enum, default_value_t = InputFormatArg::EnvelopeJsonl)]
        input_format: InputFormatArg,
        #[arg(long, value_enum, default_value_t = OutputFormat::Terminal)]
        format: OutputFormat,
        #[arg(long)]
        synthetic_evidence_manifest: Option<PathBuf>,
        #[arg(long, default_value_t = 64 * 1024 * 1024)]
        max_file_bytes: u64,
        #[arg(long, default_value_t = 1_000_000)]
        max_records: usize,
        #[arg(long, default_value_t = 1024 * 1024)]
        max_payload_bytes: usize,
        #[arg(long, default_value_t = 64)]
        max_json_depth: usize,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum InputFormatArg {
    CanonicalJsonl,
    EnvelopeJsonl,
    EnvelopeArray,
}

impl From<InputFormatArg> for ImportFormat {
    fn from(value: InputFormatArg) -> Self {
        match value {
            InputFormatArg::CanonicalJsonl => Self::CanonicalJsonl,
            InputFormatArg::EnvelopeJsonl => Self::MessageEnvelopeJsonl,
            InputFormatArg::EnvelopeArray => Self::MessageEnvelopeArray,
        }
    }
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormat {
    Json,
    Terminal,
}

#[derive(Debug, Serialize)]
struct CliReport<'a> {
    tool: &'static str,
    tool_version: &'static str,
    report_schema: &'static str,
    build: BuildIdentity,
    vda_version: &'a str,
    report_validity: ReportValidity,
    source_digest: &'a str,
    import_summary: PublicImportSummary,
    findings: &'a [Finding],
}

#[derive(Debug, Serialize)]
struct BuildIdentity {
    commit: &'static str,
    dependency_lock_sha256: &'static str,
    rule_catalog_sha256: &'static str,
    protocol_bundle_id: &'static str,
}

const fn build_identity() -> BuildIdentity {
    BuildIdentity {
        commit: env!("VDA5050_BUILD_COMMIT"),
        dependency_lock_sha256: env!("VDA5050_LOCK_SHA256"),
        rule_catalog_sha256: env!("VDA5050_RULE_CATALOG_SHA256"),
        protocol_bundle_id: PROTOCOL_BUNDLE_ID,
    }
}

#[derive(Debug, Serialize)]
struct PublicImportSummary {
    #[serde(rename = "imported_records")]
    imported: usize,
    #[serde(rename = "rejected_records")]
    rejected: usize,
    #[serde(rename = "gap_records")]
    gaps: usize,
    #[serde(rename = "non_message_records")]
    non_messages: usize,
    #[serde(rename = "unknown_records")]
    unknown: usize,
    #[serde(rename = "truncated_records")]
    truncated: usize,
    retained_bytes: u64,
    retained_bytes_limit: u64,
    retained_budget_exhausted: bool,
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("vda5050-doctor: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), String> {
    match cli.command {
        Command::Diagnose {
            input,
            vda_version,
            input_format,
            format,
            synthetic_evidence_manifest,
            max_file_bytes,
            max_records,
            max_payload_bytes,
            max_json_depth,
        } => {
            if vda_version == "2.1.0" {
                return Err("VDA 2.1.0 rule profile is not implemented; use 3.0.0 only".to_owned());
            }
            if vda_version != "3.0.0" {
                return Err("unsupported VDA version; expected 3.0.0".to_owned());
            }
            let config = ImportConfig {
                max_file_bytes,
                max_records,
                max_payload_bytes,
                max_json_depth,
            };
            if max_file_bytes == 0
                || max_records == 0
                || max_payload_bytes == 0
                || max_json_depth == 0
            {
                return Err("all import limits must be greater than zero".to_owned());
            }
            let imported = import_path(&input, input_format.into(), config)
                .map_err(|error| classify_import_error(&error.to_string()))?;
            let evidence = synthetic_evidence_manifest
                .as_deref()
                .map(load_synthetic_evidence_manifest)
                .transpose()
                .map_err(|error| error.to_string())?;
            let validity = report_validity(&imported);
            let context = trace_context_with_synthetic_evidence(&imported, evidence.as_ref())
                .map_err(|error| error.to_string())?;
            let diagnosed = analyze(&context);

            match format {
                OutputFormat::Json => write_json(&vda_version, validity, &imported, &diagnosed),
                OutputFormat::Terminal => write_terminal(
                    &vda_version,
                    validity,
                    &imported,
                    &diagnosed,
                    evidence.is_some(),
                ),
            }
        }
    }
}

fn report_validity(imported: &ImportReport) -> ReportValidity {
    if imported.summary.rejected > 0
        || imported.summary.gaps > 0
        || imported.summary.truncated > 0
        || imported.summary.unknown > 0
    {
        ReportValidity::Partial
    } else {
        ReportValidity::Valid
    }
}

fn public_summary(imported: &ImportReport) -> PublicImportSummary {
    let retained_budget_exhausted = imported.records.iter().any(|record| {
        matches!(
            &record.kind,
            RecordKind::Rejected {
                reason: RejectReason::RetainedBytesLimitExceeded { .. }
            }
        )
    });
    PublicImportSummary {
        imported: imported.summary.imported,
        rejected: imported.summary.rejected,
        gaps: imported.summary.gaps,
        non_messages: imported.summary.non_messages,
        unknown: imported.summary.unknown,
        truncated: imported.summary.truncated,
        retained_bytes: imported.summary.retained_bytes,
        retained_bytes_limit: imported.summary.retained_bytes_limit,
        retained_budget_exhausted,
    }
}

fn write_json(
    vda_version: &str,
    validity: ReportValidity,
    imported: &ImportReport,
    diagnosed: &DiagnosticReport,
) -> Result<(), String> {
    let report = CliReport {
        tool: "vda5050-doctor",
        tool_version: env!("CARGO_PKG_VERSION"),
        report_schema: "vda5050-lab.doctor-report/1",
        build: build_identity(),
        vda_version,
        report_validity: validity,
        source_digest: &imported.source_digest,
        import_summary: public_summary(imported),
        findings: &diagnosed.findings,
    };
    let output = to_canonical_json_value(&report)
        .map_err(|_| "failed to serialize the canonical report".to_owned())?;
    println!("{output}");
    Ok(())
}

fn write_terminal(
    vda_version: &str,
    validity: ReportValidity,
    imported: &ImportReport,
    diagnosed: &DiagnosticReport,
    synthetic_evidence: bool,
) -> Result<(), String> {
    println!(
        "vda5050-doctor {} — VDA {vda_version} — build {}",
        env!("CARGO_PKG_VERSION"),
        env!("VDA5050_BUILD_COMMIT")
    );
    println!(
        "report={validity:?} imported={} rejected={} gaps={} truncated={} retained={}/{}",
        imported.summary.imported,
        imported.summary.rejected,
        imported.summary.gaps,
        imported.summary.truncated,
        imported.summary.retained_bytes,
        imported.summary.retained_bytes_limit
    );
    if synthetic_evidence {
        println!("evidence=TIER1_SYNTHETIC_SAME_JOB (not production proof)");
    }
    if diagnosed.findings.is_empty() {
        println!("No supported incident finding was produced from the supplied observations.");
        return Ok(());
    }

    for finding in &diagnosed.findings {
        let incident = to_incident_report(finding, validity, imported, synthetic_evidence);
        let limits = RenderLimits::new(16 * 1024, 2 * 1024)
            .map_err(|_| "invalid internal terminal limits".to_owned())?;
        let rendered = render_terminal(&incident, limits);
        println!("{}", rendered.text());
    }
    Ok(())
}

fn to_incident_report(
    finding: &Finding,
    validity: ReportValidity,
    imported: &ImportReport,
    synthetic_evidence: bool,
) -> IncidentReport {
    let observation = Observation::new(
        format!("observation/{}", finding.rule_id),
        "The listed events support this diagnostic result.",
        finding.evidence_event_ids.clone(),
    );
    let inference = Inference::new(
        format!("inference/{}", finding.rule_id),
        finding.summary.clone(),
        [format!("observation/{}", finding.rule_id)],
        if finding.evaluation.verdict() == vda5050_core::Verdict::Inconclusive {
            InferenceConfidence::Tentative
        } else {
            InferenceConfidence::Supported
        },
    );
    let recommendations = finding
        .next_actions
        .iter()
        .enumerate()
        .map(|(index, action)| {
            Recommendation::new(
                format!("recommendation/{}/{index}", finding.rule_id),
                action.clone(),
                finding.investigation_target,
                finding.missing_evidence.clone(),
            )
        })
        .collect();
    let specifications = finding
        .authority
        .as_ref()
        .map_or_else(Vec::new, |authority| {
            vec![SpecificationReference::new(
                format!("{}-{}", authority.kind, authority.version),
                authority.section.clone(),
                authority.artifact_digest.clone(),
            )]
        });
    let evidence = finding
        .evidence_event_ids
        .iter()
        .filter_map(|event_id| {
            imported
                .records
                .iter()
                .find(|record| record.event_id == *event_id)
                .map(|record| {
                    EvidenceReference::new(
                        event_id.clone(),
                        Some(record.location.byte_offset),
                        record.raw_digest.clone(),
                        "Imported trace record",
                    )
                })
        })
        .collect();

    let assertions = if synthetic_evidence {
        vec![Assertion::new(
            "assertion/tier1-synthetic-run",
            "Actor, participant, session epoch, and completeness mappings were supplied by the isolated same-job demo manifest.",
            "vda5050-lab.synthetic-evidence/1",
            AssertionTrust::Verified,
        )]
    } else {
        Vec::new()
    };

    IncidentReport::new(
        finding.rule_id.clone(),
        validity,
        finding.diagnostic_domain,
        finding.evaluation,
        finding.finding_severity,
        finding.summary.clone(),
    )
    .with_evidence_layers(
        vec![observation],
        assertions,
        vec![inference],
        recommendations,
    )
    .with_specification_references(specifications)
    .with_evidence_references(evidence)
    .with_missing_evidence(finding.missing_evidence.clone())
}

fn classify_import_error(error: &str) -> String {
    let lower = error.to_ascii_lowercase();
    if lower.contains("symlink") {
        "input rejected: symlink paths are not allowed".to_owned()
    } else if lower.contains("regular file") || lower.contains("device") {
        "input rejected: expected a regular local file".to_owned()
    } else if lower.contains("file byte limit") {
        "input rejected: file byte limit exceeded".to_owned()
    } else {
        "input rejected by the bounded offline importer".to_owned()
    }
}
