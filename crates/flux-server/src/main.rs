use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
   /// Config file path
   #[arg(short, long, default_value = "config.toml")]
   config: String,
}

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt::init();
    
    let args = Args::parse();
    tracing::info!("Starting FLUX IOT Server with config: {}", args.config);

    flux_core::init();
    
    // TODO: Init Axum server
}
