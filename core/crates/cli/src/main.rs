use std::env;

fn main() {
    std::process::exit(touch_browser_cli::run_cli_main(
        env::args().skip(1).collect::<Vec<_>>(),
    ));
}
