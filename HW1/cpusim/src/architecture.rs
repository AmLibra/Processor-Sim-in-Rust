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
const ALLOWED_OP_CODES: [&str; 5] = ["add", "sub", "mulu", "divu", "remu"];

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

struct DecodedInstruction {
    op_code: String,
    logical_destination: u8,
    op_a_reg_tag: u8,
    op_b_reg_tag: u8,
}

impl DecodedInstruction {
    fn new(
        op_code: String,
        logical_destination: u8,
        op_a_reg_tag: u8,
        op_b_reg_tag: u8,
    ) -> DecodedInstruction {
        DecodedInstruction {
            op_code,
            logical_destination,
            op_a_reg_tag,
            op_b_reg_tag,
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
    decoded_instructions: Vec<DecodedInstruction>,
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
        if backpressure || self.exception {
            // Apply backpressure or handle exception by not fetching new instructions
            return;
        }

        while self.decoded_instructions.len() < DECODED_BUFFER_SIZE && !instructions.is_empty() {
            if let Some(instruction) = instructions.pop() {
                self.decoded_pcs.push(self.pc);
                self.decoded_instructions.push(self.decode(instruction));
                self.pc += 1; // Increment PC for each fetched instruction
            }
        }
    }

    /// Decodes an assembly instruction string into its components.
    ///
    /// ex: "add x0, x1, x2" -> ("add", 0, 1, 2)
    fn decode(instruction: &str) -> Result<DecodedInstruction, &'static str> {
        let parts: Vec<&str> = instruction.split_whitespace().collect();
        if parts.len() != 4 {
            return Err("Invalid instruction format");
        }

        let op_code = parts[0].to_string();
        if !ALLOWED_OP_CODES.contains(&op_code.as_str()) {
            return Err("Invalid operation code");
        }

        let logical_destination = parts[1][1..]
            .parse::<u8>()
            .map_err(|_| "Invalid destination register")?;
        let op_a_reg_tag = parts[2][1..]
            .parse::<u8>()
            .map_err(|_| "Invalid source register A")?;
        let op_b_reg_tag = parts[3][1..]
            .parse::<u8>()
            .map_err(|_| "Invalid source register B")?;

        Ok(DecodedInstruction::new(
            op_code,
            logical_destination,
            op_a_reg_tag,
            op_b_reg_tag,
        ))
    }

    /// Performs the rename and dispatch process for the decoded instructions.
    fn rename_and_dispatch(&mut self) -> bool {
        let backpressure = true;
        if self.decoded_instructions.is_empty() {
            return !backpressure;
        }
        if !self.has_sufficient_resources() {
            return backpressure;
        }

        for decoded_instruction in self.decoded_instructions {
            let physical_dest_register =
                self.allocate_physical_register(decoded_instruction.logical_destination);
            let old_dest_register = self.register_map_table[decoded_instruction.logical_destination as usize];

            let physical_op_a_reg_tag = self.register_map_table[decoded_instruction.op_a_reg_tag as usize];
            let op_a_ready = self.register_is_ready(decoded_instruction.op_a_reg_tag);
            let physical_op_b_reg_tag = self.register_map_table[decoded_instruction.op_b_reg_tag as usize];
            let op_b_ready = self.register_is_ready(decoded_instruction.op_b_reg_tag);

            // TODO: no care for value if not ready, check if this is correct
            let integer_queue_entry = IntegerQueueEntry::new(
                physical_dest_register,
                op_a_ready,
                physical_op_a_reg_tag,
                self.physical_register_file[physical_op_a_reg_tag],
                op_b_ready,
                physical_op_b_reg_tag,
                self.physical_register_file[physical_op_b_reg_tag],
                decoded_instruction.op_code,
                self.pc,
            );
            self.integer_queue.push(integer_queue_entry);

            let active_list_entry = ActiveListEntry::new(
                false,
                false,
                decoded_instruction.logical_destination,
                old_dest_register,
                self.pc, // TODO: this should be the PC of the instruction, not the current PC
            );
            self.active_list.push(active_list_entry);

            // TODO: set busy bit for all read registers
            self.set_busy_bit(physical_dest_register, true);
        }

        !backpressure
    }

    fn has_sufficient_resources(&self) -> bool {
        self.free_list.len() >= DECODED_BUFFER_SIZE
            && self.active_list.len() + DECODED_BUFFER_SIZE <= ACTIVE_LIST_SIZE
            && self.integer_queue.len() + DECODED_BUFFER_SIZE <= INTEGER_QUEUE_SIZE
    }

    /// Checks if busy bit is set for a register.
    fn register_is_ready(&self, register: u8) -> bool {
        self.busy_bit_table[register as usize] == false
    }

    fn allocate_physical_register(&mut self, logical_register: u8) -> u8 {
        let physical_register = self.free_list.pop().unwrap(); // TODO: not sure of allocation policy
        self.register_map_table[logical_register as usize] = physical_register;
        physical_register
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
