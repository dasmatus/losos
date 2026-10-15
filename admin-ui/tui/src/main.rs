use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let mut io = losos::commands::Io::real();
    ExitCode::from(losos::commands::run(&args, &mut io))
}
