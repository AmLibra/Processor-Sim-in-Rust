use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn Error>> {
    let mut instructions = parse_input()?;

    while !instructions.is_empty() {
        let instruction = instructions.pop().unwrap();
        println!("{}", instruction);
    }

    Ok(())
}

fn parse_input() -> Result<Vec<String>, Box<dyn Error>> {
    let input_file = resolve_input_path()?;
    // Convert PathBuf to a string slice for read_to_string
    let json_data = fs::read_to_string(input_file.as_path())?;
    let mut instructions: Vec<String> = serde_json::from_str(&json_data)?;
    instructions.reverse();
    Ok(instructions)
}

fn resolve_input_path() -> Result<PathBuf, Box<dyn Error>> {
    let mut input_path = PathBuf::from(env::current_dir()?);
    // Navigate up two directories to get to `cs470`
    input_path.pop(); // Move up from `src` to `cpusim`
    input_path.pop(); // Move up from `cpusim` to `cs470`
    input_path.push(env::args().nth(1).unwrap());
    Ok(input_path)
}
