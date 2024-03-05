use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

pub mod architecture;

fn main() -> Result<(), Box<dyn Error>> {
    let _instructions = parse_input()?;

    // Initialize the processor state
    let mut state_log: Vec<architecture::ProcessorState> = Vec::new();
    let mut processor_state = architecture::init_processor_state();

    // Log the initial state
    processor_state.log(&mut state_log);

    processor_state.add_active_list_entry(architecture::ActiveListEntry::new());
    processor_state.add_integer_queue_entry(architecture::IntegerQueueEntry::new());
    processor_state.set_busy_bit(0, true);
    processor_state.log(&mut state_log);

    save_log(&state_log)?;

    Ok(())
}

fn parse_input() -> Result<Vec<String>, Box<dyn Error>> {
    let input_file = resolve_input_path()?;
    let json_data = fs::read_to_string(input_file.as_path())?;
    let mut instructions: Vec<String> = serde_json::from_str(&json_data)?;
    instructions.reverse();
    Ok(instructions)
}

fn save_log(state_log: &Vec<architecture::ProcessorState>) -> Result<(), Box<dyn Error>> {
    let output_file = resolve_output_path()?;
    match serde_json::to_string_pretty(state_log) {
        Ok(json) => fs::write(output_file.as_path(), json)?,
        Err(e) => eprintln!("Error serializing processor state: {}", e),
    }
    Ok(())
}

fn resolve_path(arg_index: usize) -> Result<PathBuf, Box<dyn Error>> {
    let mut path = PathBuf::from(env::current_dir()?);
    // Navigate up two directories to get to `cs470`
    path.pop(); // Move up from `src` to `cpusim`
    path.pop(); // Move up from `cpusim` to `cs470`

    // Retrieve the argument at `arg_index` and append it to the path
    let arg = env::args()
        .nth(arg_index)
        .ok_or("Expected argument not found")?;
    path.push(arg);
    Ok(path)
}

fn resolve_input_path() -> Result<PathBuf, Box<dyn Error>> {
    resolve_path(1)
}

fn resolve_output_path() -> Result<PathBuf, Box<dyn Error>> {
    resolve_path(2)
}
