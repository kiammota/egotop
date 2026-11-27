use std::process;

fn main() {
    match egotop::cli::run() {
        Ok(0) => {}
        Ok(code) => process::exit(code),
        Err(err) => {
            eprintln!("egotop error: {err:#}");
            process::exit(1);
        }
    }
}
