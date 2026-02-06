use serde::Serialize;

pub fn print_json<T: Serialize + ?Sized>(data: &T) {
    match serde_json::to_string_pretty(data) {
        Ok(json) => println!("{json}"),
        Err(e) => {
            eprintln!("Error serializing to JSON: {e}");
            std::process::exit(1);
        }
    }
}
