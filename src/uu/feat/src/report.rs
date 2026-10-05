// How feat tells its caller what happened: lines for a person at a terminal,
// or, with --driven, JSON Lines events for a program (Feature Manager is the
// first). The events are the part of peipkg's driven vocabulary that applies
// to feat: progress, message, and one terminal done or error. feat asks no
// questions, so nothing is read back.

use std::io::{self, Write};

use serde_json::{json, Value};

use crate::error::Error;
use crate::feature::Phase;
use crate::registry::State;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Report {
    Terminal,
    Driven,
}

impl Report {
    pub fn driven(self) -> bool {
        self == Report::Driven
    }

    /// What feat did or found, such as "installed dynamic-boot".
    pub fn said(self, line: &str) {
        match self {
            Report::Terminal => println!("feat: {line}"),
            Report::Driven => event(&json!({"event": "message", "text": line})),
        }
    }

    /// A lifecycle script is about to run, `step` of `steps`.
    pub fn progress(self, phase: Phase, step: usize, steps: usize) {
        if self.driven() {
            event(&json!({"event": "progress", "phase": phase.as_str(),
                "step": step, "steps": steps}));
        }
    }

    /// A line a lifecycle script wrote. At a terminal the script writes
    /// there itself, so this is only ever called when driven.
    pub fn output(self, line: &str) {
        event(&json!({"event": "message", "text": line}));
    }

    /// The terminal event of a driven change: what it came to, and the
    /// state it left the feature in.
    pub fn done(self, summary: &str, state: State) {
        if self.driven() {
            event(&json!({"event": "done", "summary": summary, "state": state.label()}));
        }
    }

    /// The terminal event of a driven change that failed.
    pub fn error(self, err: &Error) {
        event(&json!({"event": "error", "code": err.code(), "message": err.to_string()}));
    }
}

/// Write one event as a line. A caller that has gone away loses the events,
/// but not the change: the write fails quietly (SIGPIPE is ignored when
/// driven) and the scripts run on.
pub fn event(v: &Value) {
    let mut out = io::stdout().lock();
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}
