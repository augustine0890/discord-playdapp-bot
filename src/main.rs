mod config;

fn main() {
    let config = config::Config::new("config.yaml").expect("Failed to read configuration file");

    println!("{:?}", config);
}
