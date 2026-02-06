pub mod json;
pub mod table;

use crate::cli::OutputFormatArg;
use serde::Serialize;
use std::io::IsTerminal;

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

/// Print a single value in the requested format.
pub fn print_output<T: Serialize + TableDisplay>(data: &T, format: OutputFormat) {
    match format {
        OutputFormat::Json => json::print_json(data),
        OutputFormat::Table => {
            let table = data.to_table();
            println!("{table}");
        }
    }
}

/// Print a slice of items in the requested format.
pub fn print_list<T: Serialize>(items: &[T], table: &dyn TableDisplay, format: OutputFormat) {
    match format {
        OutputFormat::Json => json::print_json(items),
        OutputFormat::Table => {
            let t = table.to_table();
            println!("{t}");
        }
    }
}

/// Print a plain number (e.g., workflow count).
pub fn print_count(count: u64, format: OutputFormat) {
    match format {
        OutputFormat::Json => {
            println!("{}", serde_json::json!({ "count": count }));
        }
        OutputFormat::Table => {
            println!("{count}");
        }
    }
}
