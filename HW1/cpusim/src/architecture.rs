use serde::Serialize;

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
    pub pc: u32,
}

impl ActiveListEntry {
    pub fn new() -> ActiveListEntry {
        ActiveListEntry {
            done: false,
            exception: false,
            logical_destination: 0,
            old_destination: 0,
            pc: 0,
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
    pub op_a_value: u32,
    #[serde(rename = "OpBIsReady")]
    pub op_b_is_ready: bool,
    #[serde(rename = "OpBRegTag")]
    pub op_b_reg_tag: u8,
    #[serde(rename = "OpBValue")]
    pub op_b_value: u32,
    #[serde(rename = "OpCode")]
    pub op_code: String,
    #[serde(rename = "PC")]
    pub pc: u32,
}

impl IntegerQueueEntry {
    pub fn new() -> IntegerQueueEntry {
        IntegerQueueEntry {
            dest_register: 0,
            op_a_is_ready: false,
            op_a_reg_tag: 0,
            op_a_value: 0,
            op_b_is_ready: false,
            op_b_reg_tag: 0,
            op_b_value: 0,
            op_code: String::from(""),
            pc: 0,
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
    decoded_pcs: Vec<u32>,
    #[serde(rename = "Exception")]
    exception: bool,
    #[serde(rename = "ExceptionPC")]
    exception_pc: u32,
    #[serde(rename = "FreeList")]
    free_list: Vec<u8>,
    #[serde(rename = "IntegerQueue")]
    integer_queue: Vec<IntegerQueueEntry>,
    #[serde(rename = "PC")]
    pc: u32,
    #[serde(rename = "PhysicalRegisterFile")]
    physical_register_file: Vec<u32>,
    #[serde(rename = "RegisterMapTable")]
    register_map_table: Vec<u8>,
}

impl ProcessorState {
    fn new() -> ProcessorState {
        ProcessorState {
            active_list: Vec::new(),
            busy_bit_table: vec![false; 64],
            decoded_pcs: Vec::new(),
            exception: false,
            exception_pc: 0,
            free_list: (32..64).collect(),
            integer_queue: Vec::new(),
            pc: 0,
            physical_register_file: vec![0; 64],
            register_map_table: (0..32).collect(),
        }
    }

    pub fn log(&self, state_log: &mut Vec<ProcessorState>) {
        state_log.push(self.clone());
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
