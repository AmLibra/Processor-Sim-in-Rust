use serde::Serialize;

const ALLOWED_OP_CODES: [&str; 5] = ["add", "sub", "mulu", "divu", "remu"];
const IMMEDIATE_OP_CODES: [&str; 1] = ["addi"];

#[derive(Clone, PartialEq, Serialize)]
pub struct ActiveListEntry {
    #[serde(rename = "Done")]
    pub is_done: bool,
    #[serde(rename = "Exception")]
    pub is_exception: bool,
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
            is_done: done,
            is_exception: exception,
            logical_destination,
            old_destination,
            pc,
        }
    }
}

#[derive(Clone, PartialEq, Serialize)]
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
        op_b_reg_tag: u8, // u32 to handle immediate values
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

    pub fn is_ready(&self) -> bool {
        self.op_a_is_ready && self.op_b_is_ready
    }
}

#[derive(Clone, PartialEq, Serialize)]
pub struct ALUEntry {
    dest_register: u8,
    op_a_value: u64,
    op_b_value: u64,
    op_code: String,
    pc: u64,
}

impl ALUEntry {
    pub fn new(
        dest_register: u8,
        op_a_value: u64,
        op_b_value: u64,
        op_code: String,
        pc: u64,
    ) -> ALUEntry {
        ALUEntry {
            dest_register,
            op_a_value,
            op_b_value,
            op_code,
            pc,
        }
    }
}

#[derive(Clone, PartialEq, Serialize)]
pub struct CommitBufferEntry {
    pub dest_register: u8,
    pub value: u64,
    pub pc: u64,
}

impl CommitBufferEntry {
    pub fn new(dest_register: u8, value: u64, pc: u64) -> CommitBufferEntry {
        CommitBufferEntry {
            dest_register,
            value,
            pc,
        }
    }
}

#[derive(Clone, PartialEq, Serialize)]
pub struct ALU {
    stage1: Option<ALUEntry>,
    stage2: Option<ALUEntry>,
    pub is_forwarding: bool,
    pub forwarding_reg: u8,
    pub forwarding_value: u64,
    pub forwarding_pc: u64,
    pub forwarding_exception: bool,
}

impl ALU {
    pub fn new() -> ALU {
        ALU {
            stage1: None,
            stage2: None,
            is_forwarding: false,
            forwarding_reg: 0,
            forwarding_value: 0,
            forwarding_pc: 0,
            forwarding_exception: false,
        }
    }

    pub fn is_busy(&self) -> bool {
        self.stage1.is_some()
    }

    pub fn latch(&mut self, entry: IntegerQueueEntry) {
        if !self.is_busy() {
            self.stage1 = Some(ALUEntry::new(
                entry.dest_register,
                entry.op_a_value,
                entry.op_b_value,
                entry.op_code,
                entry.pc,
            ));
        } else {
            panic!("ALU stage 1 is already occupied");
        }
    }

    pub fn execute(&mut self) {
        if self.stage2.is_some() {
            self.stage2 = None;
        }
        if self.stage1.is_some() {
            self.stage2 = self.stage1.take();
            self.update_forwarding(); // Update forwarding values directly after stage 2 is occupied
        }
    }

    fn compute(&mut self, stage1_entry: &ALUEntry) -> u64 {
        match stage1_entry.op_code.as_str() {
            "add" => stage1_entry.op_a_value + stage1_entry.op_b_value,
            "sub" => if stage1_entry.op_a_value < stage1_entry.op_b_value {
                self.forwarding_exception = true;
                return 0;
            } else {
                stage1_entry.op_a_value - stage1_entry.op_b_value
            },
            "mulu" => stage1_entry.op_a_value * stage1_entry.op_b_value,
            "divu" => {
                if stage1_entry.op_b_value == 0 {
                    self.forwarding_exception = true;
                    return 0;
                } else {
                    stage1_entry.op_a_value / stage1_entry.op_b_value
                }
            }
            "remu" => {
                if stage1_entry.op_b_value == 0 {
                    self.forwarding_exception = true;
                    return 0;
                } else {
                    stage1_entry.op_a_value % stage1_entry.op_b_value
                }
            }
            _ => panic!("Invalid op code"),
        }
    }

    fn update_forwarding(&mut self) {
        let stage2_entry = self.stage2.as_ref().unwrap().clone();
        self.is_forwarding = true;
        self.forwarding_reg = stage2_entry.dest_register;
        self.forwarding_pc = stage2_entry.pc;
        self.forwarding_value = self.compute(&stage2_entry);
    }
}

#[derive(Clone, PartialEq)]
pub struct DecodedInstruction {
    pub pc: u64,
    pub op_code: String,
    pub immediate: bool,
    pub logical_destination: u8,
    pub op_a_reg_tag: u8,
    pub op_b_reg_tag: u8,
    pub immediate_value: u32,
}

impl DecodedInstruction {
    pub fn new(
        pc: u64,
        op_code: String,
        immediate: bool,
        logical_destination: u8,
        op_a_reg_tag: u8,
        op_b_reg_tag: u8,
        immediate_value: u32,
    ) -> DecodedInstruction {
        DecodedInstruction {
            pc,
            op_code,
            immediate,
            logical_destination,
            op_a_reg_tag,
            op_b_reg_tag,
            immediate_value,
        }
    }
}

pub struct Instruction {
    value: String,
}

impl Instruction {
    pub fn new(value: String) -> Instruction {
        Instruction { value }
    }

    /// Decodes an assembly instruction string into its components.
    ///
    /// ex: "add x0, x1, x2" -> DecodedInstruction
    /// ex: "addi x0, x1, 10" -> DecodedInstruction with immediate value
    pub fn decode(&self, pc: u64) -> Result<DecodedInstruction, &'static str> {
        let instruction_minified = self.value.replace(",", "");
        let parts: Vec<&str> = instruction_minified.split_whitespace().collect();
        if parts.len() != 4 {
            return Err("Invalid instruction format");
        }

        let mut op_code = parts[0];
        let is_immediate = IMMEDIATE_OP_CODES.contains(&op_code);

        if IMMEDIATE_OP_CODES.contains(&op_code) {
            op_code = "add"; // "addi" is treated as "add" for the purpose of this simulation
        }

        if !ALLOWED_OP_CODES.contains(&op_code) {
            return Err("Invalid op code");
        }

        let logical_destination = Instruction::parse_register(parts[1])?;
        let op_a_reg_tag = Instruction::parse_register(parts[2])?;

        let op_b_reg_tag: u8;
        let immediate_value: u32;

        if is_immediate {
            immediate_value = parts[3]
                .parse::<u32>()
                .map_err(|_| "Invalid immediate value")?;
            op_b_reg_tag = 0; // Immediate instructions don't use a second register
        } else {
            op_b_reg_tag = Instruction::parse_register(parts[3])?;
            immediate_value = 0; // Non-immediate instructions don't have an immediate value
        }

        Ok(DecodedInstruction::new(
            pc,
            op_code.to_string(),
            is_immediate,
            logical_destination,
            op_a_reg_tag,
            op_b_reg_tag,
            immediate_value,
        ))
    }

    /// Parses a register string (e.g., "x1") and returns the register number.
    fn parse_register(reg_str: &str) -> Result<u8, &'static str> {
        reg_str[1..]
            .parse::<u8>()
            .map_err(|_| "Invalid register identifier")
    }
}
