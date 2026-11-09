fn main() {
    let args: Vec<String> = std::env::args().collect();
    let count = args.get(1).map(|s| s.parse::<usize>()).transpose();
    let frames = args.get(2).map(|s| s.parse::<usize>()).transpose();
    let result = match (count, frames) {
        (Ok(count), Ok(frames)) => {
            particles::benchmark::run(count.unwrap_or(3000), frames.unwrap_or(600))
        }
        _ => Err("Usage: benchmark [particle-count] [frames]".into()),
    };
    match result {
        Ok(report) => print!("{report}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
