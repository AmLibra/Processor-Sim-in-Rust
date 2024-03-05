use std::io::SeekFrom::Start;
use serde::Serialize;

const INITIAL_PC: u64 = 0;
const INITIAL_EXCEPTION_PC: u64 = 0;
const INTEGER_QUEUE_SIZE: usize = 32;
const ACTIVE_LIST_SIZE: usize = 32;
const BUSY_BIT_TABLE_SIZE: usize = 64;
const PHYSICAL_REGISTER_FILE_SIZE: usize = 64;
const REGISTER_MAP_TABLE_SIZE: u8 = 32;
const START_OF_FREE_REGISTER_LIST: u8 = 32;
const END_OF_FREE_REGISTER_LIST: u8 = 64;
const DECODED_BUFFER_SIZE: usize = 4;
const INITIAL_EXCEPTION_STATE: bool = false;

#[derive(Clone, Serialize)]
pub struct ActiveListEntry {
    #[serde(rename = "Done")]
    pub done: bool,
    #[serde(rename = "Exception")]
    pub exception: bool,
    #[serde(rename = "LogicalDestination")]
    pub logical_destination: u8,
    #[serde(rename = "OldDestination")]
    pub old_destination: u8,
    #[serde(rename = "PC")]
    pub pc: u64,
}

impl ActiveListEntry {
    pub fn new(
        done: bool,
        exception: bool,
        logical_destination: u8,
        old_destination: u8,
        pc: u64,
    ) -> ActiveListEntry {
        ActiveListEntry {
            done,
            exception,
            logical_destination,
            old_destination,
            pc,
        }
    }
}

#[derive(Clone, Serialize)]
pub struct IntegerQueueEntry {
    #[serde(rename = "DestRegister")]
    pub dest_register: u8,
    #[serde(rename = "OpAIsReady")]
    pub op_a_is_ready: bool,
    #[serde(rename = "OpARegTag")]
    pub op_a_reg_tag: u8,
    #[serde(rename = "OpAValue")]
    pub op_a_value: u64,
    #[serde(rename = "OpBIsReady")]
    pub op_b_is_ready: bool,
    #[serde(rename = "OpBRegTag")]
    pub op_b_reg_tag: u8,
    #[serde(rename = "OpBValue")]
    pub op_b_value: u64,
    #[serde(rename = "OpCode")]
    pub op_code: String,
    #[serde(rename = "PC")]
    pub pc: u64,
}

impl IntegerQueueEntry {
    pub fn new(
        dest_register: u8,
        op_a_is_ready: bool,
        op_a_reg_tag: u8,
        op_a_value: u64,
        op_b_is_ready: bool,
        op_b_reg_tag: u8,
        op_b_value: u64,
        op_code: String,
        pc: u64,
    ) -> IntegerQueueEntry {
        IntegerQueueEntry {
            dest_register,
            op_a_is_ready,
            op_a_reg_tag,
            op_a_value,
            op_b_is_ready,
            op_b_reg_tag,
            op_b_value,
            op_code,
            pc,
        }
    }
}

#[derive(Clone, Serialize)]
pub struct ProcessorState {
    #[serde(rename = "ActiveList")]
    active_list: Vec<ActiveListEntry>,
    #[serde(rename = "BusyBitTable")]
    busy_bit_table: Vec<bool>,
    #[serde(rename = "DecodedPCs")]
    decoded_pcs: Vec<u64>,
    #[serde(skip_serializing)] // skip serializing decoded instructions
    decoded_instructions: Vec<String>,
    #[serde(rename = "Exception")]
    exception: bool,
    #[serde(rename = "ExceptionPC")]
    exception_pc: u64,
    #[serde(rename = "FreeList")]
    free_list: Vec<u8>, // FIFO queue
    #[serde(rename = "IntegerQueue")]
    integer_queue: Vec<IntegerQueueEntry>,
    #[serde(rename = "PC")]
    pc: u64,
    #[serde(rename = "PhysicalRegisterFile")]
    physical_register_file: Vec<u64>,
    #[serde(rename = "RegisterMapTable")]
    register_map_table: Vec<u8>,
}

impl ProcessorState {
    fn new() -> ProcessorState {
        ProcessorState {
            active_list: Vec::with_capacity(ACTIVE_LIST_SIZE),
            busy_bit_table: vec![false; BUSY_BIT_TABLE_SIZE],
            decoded_pcs: Vec::with_capacity(DECODED_BUFFER_SIZE),
            decoded_instructions: Vec::with_capacity(DECODED_BUFFER_SIZE),
            exception: INITIAL_EXCEPTION_STATE,
            exception_pc: INITIAL_EXCEPTION_PC,
            free_list: (START_OF_FREE_REGISTER_LIST..END_OF_FREE_REGISTER_LIST).collect(),
            integer_queue: Vec::with_capacity(INTEGER_QUEUE_SIZE),
            pc: INITIAL_PC,
            physical_register_file: vec![0; PHYSICAL_REGISTER_FILE_SIZE],
            register_map_table: (0..REGISTER_MAP_TABLE_SIZE).collect(),
        }
    }

    pub fn active_list_is_empty(&self) -> bool {
        self.active_list.is_empty()
    }

    pub fn log(&self, state_log: &mut Vec<ProcessorState>) {
        state_log.push(self.clone());
    }

    pub fn latch(&mut self, new_state: &ProcessorState) {
        *self = new_state.clone();
    }

    pub fn propagate(&self, instructions: &mut Vec<String>) -> ProcessorState {
        let mut next_state = self.clone();

        let backpressure = next_state.rename_and_dispatch();
        next_state.fetch_and_decode(instructions, backpressure);

        return next_state;
    }

    /// Fetches and decodes the next four instructions from the instruction queue.
    fn fetch_and_decode(&mut self, instructions: &mut Vec<String>, backpressure: bool) {
        // TODO: handle exceptions from commit stage and backpressure from rename and dispatch stage
        if backpressure {
            return;
        }
        for _ in 0..DECODED_BUFFER_SIZE {
            if let Some(instruction) = instructions.pop() {
                self.decoded_pcs.push(self.pc);
                self.decoded_instructions.push(instruction);
                self.pc += 1;
            }
        }
    }

    fn rename_and_dispatch(&mut self) -> bool {
        let enough_free_physical_registers = self.free_list.len() >= 2;
        let enough_integer_queue_entries = self.integer_queue.len() < INTEGER_QUEUE_SIZE;
        let enough_active_list_entries = self.active_list.len() < ACTIVE_LIST_SIZE;

        if !(enough_free_physical_registers
            && enough_integer_queue_entries
            && enough_active_list_entries)
        {
            return true;
        } else {
            return false;
        }
    }

    pub fn add_active_list_entry(&mut self, entry: ActiveListEntry) {
        self.active_list.push(entry);
    }

    pub fn add_integer_queue_entry(&mut self, entry: IntegerQueueEntry) {
        self.integer_queue.push(entry);
    }

    pub fn set_busy_bit(&mut self, register: u8, value: bool) {
        self.busy_bit_table[register as usize] = value;
    }
}

pub fn init_processor_state() -> ProcessorState {
    ProcessorState::new()
}
