use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

use crate::arch_modules::Instruction;

mod arch_modules;
pub mod architecture;

const MAX_CYCLES: usize = 50;

fn main() -> Result<(), Box<dyn Error>> {
    let mut instructions = parse_input()?;

    // Initialize the processor state
    let mut state_log: Vec<architecture::ProcessorState> = Vec::new();
    let mut processor_state = architecture::ProcessorState::new();

    // Log the initial state
    processor_state.log(&mut state_log);

    while !(instructions.is_empty() && processor_state.active_list_is_empty())
        && (state_log.len() < MAX_CYCLES)
    {
        let new_processor_state = processor_state.propagate(&mut instructions);
        processor_state.latch(&new_processor_state);
        processor_state.log(&mut state_log);
    }

    save_log(&state_log)?;

    Ok(())
}

fn parse_input() -> Result<Vec<Instruction>, Box<dyn Error>> {
    let input_file = resolve_input_path()?;
    let json_data = fs::read_to_string(input_file.as_path())?;
    let instruction_strings: Vec<String> = serde_json::from_str(&json_data)?;
    let mut instructions: Vec<Instruction> = instruction_strings
        .iter()
        .map(|x| Instruction::new(x.to_string()))
        .collect();
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
