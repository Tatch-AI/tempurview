pub mod json;
pub mod table;

use crate::cli::OutputFormatArg;
use serde::Serialize;
use std::io::{IsTerminal, Write};

pub use table::TableDisplay;

#[derive(Clone, Copy)]
pub enum OutputFormat {
    Json,
    Table,
}

impl OutputFormat {
    /// Resolve output format: explicit arg wins, otherwise table for TTY, JSON for pipe.
    pub fn resolve(explicit: Option<OutputFormatArg>) -> Self {
        match explicit {
            Some(OutputFormatArg::Json) => OutputFormat::Json,
            Some(OutputFormatArg::Table) => OutputFormat::Table,
            None => {
                if std::io::stdout().is_terminal() {
                    OutputFormat::Table
                } else {
                    OutputFormat::Json
                }
            }
        }
    }
}

/// Print a single value in the requested format (to stdout).
pub fn print_output<T: Serialize + TableDisplay>(data: &T, format: OutputFormat) {
    write_output(data, format, &mut std::io::stdout());
}

/// Write a single value in the requested format to the given writer.
pub fn write_output<T: Serialize + TableDisplay>(
    data: &T,
    format: OutputFormat,
    w: &mut (dyn Write + Send),
) {
    match format {
        OutputFormat::Json => json::write_json(data, w),
        OutputFormat::Table => {
            let table = data.to_table();
            let _ = writeln!(w, "{table}");
        }
    }
}

/// Print a slice of items in the requested format (to stdout).
pub fn print_list<T: Serialize>(items: &[T], table: &dyn TableDisplay, format: OutputFormat) {
    write_list(items, table, format, &mut std::io::stdout());
}

/// Write a slice of items in the requested format to the given writer.
pub fn write_list<T: Serialize>(
    items: &[T],
    table: &dyn TableDisplay,
    format: OutputFormat,
    w: &mut (dyn Write + Send),
) {
    match format {
        OutputFormat::Json => json::write_json(items, w),
        OutputFormat::Table => {
            let t = table.to_table();
            let _ = writeln!(w, "{t}");
        }
    }
}

/// Print a plain number (to stdout).
pub fn print_count(count: u64, format: OutputFormat) {
    write_count(count, format, &mut std::io::stdout());
}

/// Write a plain number to the given writer.
pub fn write_count(count: u64, format: OutputFormat, w: &mut (dyn Write + Send)) {
    match format {
        OutputFormat::Json => {
            let _ = writeln!(w, "{}", serde_json::json!({ "count": count }));
        }
        OutputFormat::Table => {
            let _ = writeln!(w, "{count}");
        }
    }
}
