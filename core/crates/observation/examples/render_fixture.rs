use std::{env, fs};

use touch_browser_contracts::SourceType;
use touch_browser_observation::{ObservationCompiler, ObservationInput};

fn main() {
    let mut args = env::args().skip(1);
    let html_path = args
        .next()
        .expect("usage: render_fixture <html_path> <source_url> [budget]");
    let source_url = args
        .next()
        .expect("usage: render_fixture <html_path> <source_url> [budget]");
    let budget = args
        .next()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(512);
    let html = fs::read_to_string(&html_path).expect("fixture html should be readable");

    let snapshot = ObservationCompiler
        .compile(&ObservationInput::new(
            source_url,
            SourceType::Fixture,
            html,
            budget,
        ))
        .expect("fixture snapshot should compile");

    println!(
        "{}",
        serde_json::to_string_pretty(&snapshot).expect("snapshot should serialize")
    );
}
