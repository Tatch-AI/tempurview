use serde::Serialize;
use std::io::Write;

pub fn print_json<T: Serialize + ?Sized>(data: &T) {
    write_json(data, &mut std::io::stdout());
}

pub fn write_json<T: Serialize + ?Sized>(data: &T, w: &mut (dyn Write + Send)) {
    match serde_json::to_string_pretty(data) {
        Ok(json) => {
            let _ = writeln!(w, "{json}");
        }
        Err(e) => {
            eprintln!("Error serializing to JSON: {e}");
            std::process::exit(1);
        }
    }
}
