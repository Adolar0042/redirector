use divan::Bencher;
use rand::Rng as _;
use rand::prelude::IndexedRandom as _;
use redirector::config::AppConfig;
use redirector::{get_bang, resolve, update_bangs};
use tracing::{Level, error};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_writer(std::io::stderr)
        .init();
    divan::main();
}

#[divan::bench(sample_count = 10_000)]
fn resolve_plain_query(bencher: Bencher) {
    let config = create_config();
    bencher.bench(|| resolve(&config, "just a regular search query"));
}

#[divan::bench(sample_count = 10_000)]
fn resolve_query_with_bang(bencher: Bencher) {
    let config = create_config();
    bencher.bench(|| resolve(&config, "!gh just a regular search query"));
}

#[divan::bench(sample_count = 10_000)]
fn resolve_random_generated_query(bencher: Bencher) {
    let config = create_config();
    bencher
        .with_inputs(generate_random_query)
        .bench_values(|query| resolve(&config, &query));
}

#[divan::bench(sample_count = 10_000)]
fn get_bang_random(bencher: Bencher) {
    bencher
        .with_inputs(generate_random_query)
        .bench_values(|query| {
            let _ = get_bang(&query);
        });
}

fn create_config() -> AppConfig {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let config = AppConfig::default();
        if let Err(e) = update_bangs(&config).await {
            error!("Failed to update bangs: {e}");
        }
        config
    })
}

fn generate_random_query() -> String {
    let bang_commands = [
        "!g",
        "!w",
        "!yt",
        "!gh",
        "!so",
        "!maps",
        "!reddit",
        "!images",
        "!translate",
        "",
    ];
    let words = [
        "rust",
        "cargo",
        "benchmark",
        "performance",
        "async",
        "error",
        "lock",
        "cache",
        "config",
        "update",
        "regex",
        "network",
        "query",
        "thread",
        "sync",
        "!!!!!!!!!!!",
    ];

    let mut rng = rand::rng();
    let include_bang: bool = rng.random_bool(0.5);
    if include_bang {
        // Choose a bang command from the array.
        let bang = bang_commands.choose(&mut rng).unwrap();
        let num_words = rng.random_range(2..=5);
        let mut selected_words: Vec<&str> = words
            .choose_multiple(&mut rng, num_words)
            .copied()
            .collect();
        // Insert bang into a random position.
        let insert_index = rng.random_range(0..=selected_words.len());
        selected_words.insert(insert_index, bang);
        selected_words.join(" ")
    } else {
        let num_words = rng.random_range(2..=5);
        let selected_words: Vec<&str> = words
            .choose_multiple(&mut rng, num_words)
            .copied()
            .collect();
        selected_words.join(" ")
    }
}
