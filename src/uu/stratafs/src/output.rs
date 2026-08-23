use std::io::Write;

use crate::error::{Error, Result};
use crate::inspect::{MountReport, ResolveReport, SweepEntry};

pub fn json<T: serde::Serialize>(value: &T) -> Result<()> {
    let stdout = std::io::stdout();
    let mut output = stdout.lock();
    serde_json::to_writer_pretty(&mut output, value)
        .map_err(|error| Error::Unsupported(format!("write JSON: {error}")))?;
    output
        .write_all(b"\n")
        .map_err(|error| Error::io("write JSON", error))
}

pub fn mounts(reports: &[MountReport]) {
    for (mount_index, report) in reports.iter().enumerate() {
        if mount_index != 0 {
            println!();
        }
        if report.read_only {
            println!("{} (read-only)", report.mount);
        } else {
            println!("{}", report.mount);
        }
        for stratum in &report.strata {
            let flags = if stratum.flags.is_empty() {
                String::new()
            } else {
                format!("+{}", stratum.flags.join("+"))
            };
            println!(
                "  [{}] {}{flags} ({})",
                stratum.index, stratum.path, stratum.state
            );
        }
    }
}

pub fn resolve(report: &ResolveReport) {
    println!("path:  {}", report.path);
    println!("mount: {}", report.mount);
    println!(
        "type:  {}",
        report
            .object_type
            .map_or_else(|| "absent".to_owned(), |kind| kind.to_string())
    );
    println!("strata:");
    for stratum in &report.strata {
        let kind = stratum
            .object_type
            .map_or_else(|| "absent".to_owned(), |kind| kind.to_string());
        let flags = if stratum.flags.is_empty() {
            String::new()
        } else {
            format!(" +{}", stratum.flags.join("+"))
        };
        println!(
            "  [{}] {:11} {kind:14} {}{flags}",
            stratum.index, stratum.state, stratum.object
        );
    }
    print_action("write", &report.write);
    print_action("delete", &report.delete);
}

fn print_action(label: &str, action: &crate::inspect::ActionReport) {
    match &action.target {
        Some(target) => println!("{label}: {} {target} — {}", action.action, action.reason),
        None => println!("{label}: {} — {}", action.action, action.reason),
    }
}

pub fn sweep(reports: &[SweepEntry]) {
    if reports.is_empty() {
        println!("create stratum is empty");
        return;
    }
    for report in reports {
        match &report.related_object {
            Some(related) => println!("{:<8} {} ({related})", report.state, report.path),
            None => println!("{:<8} {}", report.state, report.path),
        }
    }
}
