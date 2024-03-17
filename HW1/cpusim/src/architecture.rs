use serde::Serialize;

use crate::arch_modules::{
    ActiveListEntry, ALU, CommitBufferEntry, DecodedInstruction, Instruction, IntegerQueueEntry,
};

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
const ALU_COUNT: usize = 4;
const INITIAL_EXCEPTION_STATE: bool = false;
const EXCEPTION_PC: u64 = 0x10000;

#[derive(Clone, Serialize)]
pub struct Processor {
    #[serde(rename = "ActiveList")]
    active_list: Vec<ActiveListEntry>,
    #[serde(rename = "BusyBitTable")]
    busy_bit_table: Vec<bool>,
    #[serde(rename = "DecodedPCs")]
    decoded_pcs: Vec<u64>,
    #[serde(skip_serializing)] // skip serializing decoded instructions
    decoded_instructions: Vec<DecodedInstruction>,
    #[serde(rename = "Exception")]
    exception_mode: bool,
    #[serde(rename = "ExceptionPC")]
    exception_pc: u64,
    #[serde(rename = "FreeList")]
    free_list: Vec<u8>, // FIFO queue
    #[serde(rename = "IntegerQueue")]
    integer_queue: Vec<IntegerQueueEntry>,
    #[serde(skip_serializing)] // skip serializing ALUs
    alus: Vec<ALU>,
    #[serde(skip_serializing)] // skip serializing commit buffer
    commit_buffer: Vec<CommitBufferEntry>,
    #[serde(rename = "PC")]
    pc: u64,
    #[serde(rename = "PhysicalRegisterFile")]
    physical_register_file: Vec<u64>,
    #[serde(rename = "RegisterMapTable")]
    register_map_table: Vec<u8>,
}

impl Processor {
    pub fn new() -> Processor {
        Processor {
            active_list: Vec::with_capacity(ACTIVE_LIST_SIZE),
            busy_bit_table: vec![false; BUSY_BIT_TABLE_SIZE],
            decoded_pcs: Vec::with_capacity(DECODED_BUFFER_SIZE),
            decoded_instructions: Vec::with_capacity(DECODED_BUFFER_SIZE),
            exception_mode: INITIAL_EXCEPTION_STATE,
            exception_pc: INITIAL_EXCEPTION_PC,
            free_list: (START_OF_FREE_REGISTER_LIST..END_OF_FREE_REGISTER_LIST).collect(),
            integer_queue: Vec::with_capacity(INTEGER_QUEUE_SIZE),
            alus: vec![ALU::new(); ALU_COUNT],
            commit_buffer: Vec::with_capacity(ALU_COUNT),
            pc: INITIAL_PC,
            physical_register_file: vec![0; PHYSICAL_REGISTER_FILE_SIZE],
            register_map_table: (0..REGISTER_MAP_TABLE_SIZE).collect(),
        }
    }

    pub fn is_done(&self) -> bool {
        self.active_list.is_empty() && self.exception_mode == false
    }

    /// Logs the current state of the processor to the state log.
    pub fn log_state(&self, state_log: &mut Vec<Processor>) {
        state_log.push(self.clone());
    }

    /// Latches the current state of the processor to the given state.
    pub fn latch(&mut self, new_state: &Processor) {
        *self = new_state.clone();
    }

    /// Propagates the processor state by one cycle.
    pub fn propagate(&self, instructions: &mut Vec<Instruction>) -> Processor {
        let mut next_state = self.clone();
        let mut backpressure = false;
        next_state.commit();
        if !next_state.exception_mode {
            next_state.issue();
            backpressure = next_state.rename_and_dispatch(&self);
        }
        next_state.fetch_and_decode(instructions, backpressure);
        return next_state;
    }

    /// STAGE 1: Fetches and decodes the next four instructions from the instruction queue.
    /// 1. If backpressure is applied or an exception occurs, the fetch and decode process is halted,
    /// the PC is set to the exception PC, and the decoded instructions are cleared.
    /// 2. If the instruction queue is empty, the process is also halted.
    /// 3. Otherwise, the next up to four instructions are fetched and decoded.
    fn fetch_and_decode(&mut self, instructions: &mut Vec<Instruction>, backpressure: bool) {
        if backpressure {
            return; // Do not fetch and decode
        }
        if self.exception_mode {
            self.pc = EXCEPTION_PC;
            self.clear_decoded_instructions();
            return; // Do not fetch and decode and clear decoded instructions
        }
        while self.decoded_instructions.len() < DECODED_BUFFER_SIZE && !instructions.is_empty() {
            if let Some(instruction) = instructions.pop() {
                self.decoded_pcs.push(self.pc);
                let decoded_instruction = instruction.decode(self.pc).expect("Invalid instruction");
                self.decoded_instructions.push(decoded_instruction);
                self.pc += 1;
            }
        }
    }

    /// STAGE 2: Performs the rename and dispatch process for the decoded instructions.
    /// 1. Checks if there are enough resources to process the next four instructions.
    /// 2. If there are enough resources, renames the destination registers and dispatches the
    /// instructions to the integer queue and active list as per the R10000 CPU paper.
    /// 3. If there are not enough resources, backpressure is applied.
    /// 4. The integer queue is always listening for forwarding paths from the ALUs.
    fn rename_and_dispatch(&mut self, current_state: &Processor) -> bool {
        if !self.has_sufficient_resources() {
            return true; // Apply backpressure if resources are insufficient.
        }
        for decoded_instruction in &current_state.decoded_instructions {
            self.add_active_list_entry(decoded_instruction);
            self.add_integer_queue_entry(current_state, decoded_instruction);
        }
        self.clear_decoded_instructions();
        false // No backpressure since instructions were successfully renamed and dispatched.
    }

    /// STAGE 3: Performs the issue process for the decoded instructions.
    /// 1. Checks if the instruction is ready to be issued, prioritizing the oldest instructions,
    /// (i.e., the instructions with smaller PCs).
    /// 2. If ready, issues the instruction to an available ALU.
    /// 3. The integer queue is always listening for forwarding paths from the ALUs.
    fn issue(&mut self) {
        if self.any_alu_exception() {
            return; // Do not issue instructions if an exception is incoming.
        }
        self.read_integer_queue_fwd_paths();
        for alu in self.alus.iter_mut() {
            alu.execute();
        }
        for _ in 0..ALU_COUNT {
            self.issue_instruction();
        }
    }

    /// STAGE 4: Commits the results of the executed instructions to the physical register file.
    /// 1. Mark instructions as done or exception on receiving the results from the ALU
    /// forwarding paths.
    /// 2. Respectively, retire or rollback the instructions in the active list depending on the
    /// results.
    /// 3. Recycle the physical registers of the retired instructions, pushing them back to the
    /// free list.
    fn commit(&mut self) {
        if self.exception_mode {
            if self.active_list.is_empty() {
                self.exception_mode = false;
            }
            return self.rollback();
        }

        let mut retired_instructions = 0;
        let mut to_remove_pcs: Vec<u64> = Vec::new();

        for entry in self.clone().active_list.iter() {
            if retired_instructions == DECODED_BUFFER_SIZE {
                break; // Stop committing if four instructions are already picked.
            }
            if entry.is_exception {
                self.set_exception_mode(entry.pc);
                break;
            } else if entry.is_done {
                retired_instructions += 1;
                self.free_list.push(entry.old_destination);
                to_remove_pcs.push(entry.pc);
            } else {
                break; // Stop committing if an instruction is not completed yet.
            }
        }

        for pc in to_remove_pcs {
            self.active_list.retain(|x| x.pc != pc);
            self.commit_buffer.retain(|x| x.pc != pc);
        }
        self.read_active_list_fwd_paths();
    }

    /// EXCEPTION MODE: Rollback instructions and recover register map table, busy bit table,
    /// and free list.
    fn rollback(&mut self) {
        let mut rolled_back_instructions = 0;
        let mut to_remove_pcs: Vec<u64> = Vec::new();

        for entry in self.clone().active_list.iter().rev() {
            if rolled_back_instructions == DECODED_BUFFER_SIZE {
                break; // Stop rolling back if four instructions are already picked.
            }
            rolled_back_instructions += 1;
            let allocated_register = self.map_register(entry.logical_destination);
            self.set_free(allocated_register);
            self.free_list.push(allocated_register);
            self.register_map_table[entry.logical_destination as usize] = entry.old_destination;
            to_remove_pcs.push(entry.pc);
        }

        for pc in to_remove_pcs {
            self.active_list.retain(|x| x.pc != pc);
            self.commit_buffer.retain(|x| x.pc != pc);
        }
    }

    /// =============================================== ///
    /// --------------- Helper Functions -------------- ///
    /// =============================================== ///

    /// Clear active list entry and update register with new value
    pub fn commit_entry(&mut self, entry: ActiveListEntry) {
        let buffer_entry = self
            .commit_buffer
            .iter()
            .find(|x| x.pc == entry.pc)
            .unwrap();
        self.physical_register_file[buffer_entry.dest_register as usize] = buffer_entry.value;
        self.set_free(buffer_entry.dest_register);
    }

    /// Sets exception mode
    pub fn set_exception_mode(&mut self, pc: u64) {
        self.exception_mode = true;
        self.exception_pc = pc;
        self.reset_alus();
        self.reset_integer_queue();
    }

    /// Issues the oldest ready instruction to an available ALU.
    fn issue_instruction(&mut self) {
        let oldest_ready_instruction = self.find_oldest_ready_instruction();
        if let Some(entry) = oldest_ready_instruction {
            for alu in self.alus.iter_mut() {
                if !alu.is_busy() {
                    alu.latch(entry.clone());
                    break;
                }
            }
        }
    }

    /// Finds the oldest instruction in the integer queue that is ready to be issued.
    fn find_oldest_ready_instruction(&mut self) -> Option<IntegerQueueEntry> {
        let mut sorted_queue = self.integer_queue.clone();
        sorted_queue.sort_by(|a, b| a.pc.cmp(&b.pc));

        for entry in sorted_queue {
            if entry.is_ready() {
                self.integer_queue.retain(|x| x.pc != entry.pc);
                return Some(entry);
            }
        }
        None
    }

    /// The active list is polled for the forwarding paths from the ALUs to check if any values have
    /// been forwarded. If so, the active list updates the relevant entries with the forwarded values.
    /// The active list is also updated with the exception status of the forwarded values.
    fn read_active_list_fwd_paths(&mut self) {
        for alu in self.alus.clone().iter() {
            if alu.is_forwarding {
                self.update_active_list(alu);
            }
        }
    }

    /// The active list checks if any of its entries are ready to be issued,
    /// and if so, updates the entries accordingly.
    fn update_active_list(&mut self, alu: &ALU) {
        let mut to_commit_entries: Vec<ActiveListEntry> = Vec::new();
        for entry in self.active_list.iter_mut() {
            if entry.pc == alu.forwarding_pc {
                entry.is_done = true;
                if alu.forwarding_exception {
                    entry.is_exception = true;
                } else {
                    to_commit_entries.push(entry.clone());
                    self.commit_buffer.push(CommitBufferEntry::new(
                        alu.forwarding_reg,
                        alu.forwarding_value,
                        entry.pc,
                    ));
                }
            }
        }
        for entry in to_commit_entries {
            self.commit_entry(entry);
        }
    }

    /// Integer queue may want to know if there is an exception incoming, so poll the ALUs for that.
    fn any_alu_exception(&self) -> bool {
        for alu in self.alus.iter() {
            if alu.forwarding_exception {
                return true;
            }
        }
        false
    }

    /// The integer queue polls the forwarding paths from the ALUs to check if any values have been
    /// forwarded. If so, the integer queue updates the relevant entries with the forwarded values.
    fn read_integer_queue_fwd_paths(&mut self) {
        for alu in self.alus.clone().iter() {
            if alu.is_forwarding {
                self.update_integer_queue(alu.forwarding_reg, alu.forwarding_value);
            }
        }
    }

    /// The integer queue checks if any of its entries are ready to be issued,
    /// and if so, updates the entries accordingly.
    fn update_integer_queue(&mut self, forwarding_reg: u8, forwarding_value: u64) {
        for entry in self.integer_queue.iter_mut() {
            if !entry.op_a_is_ready && (entry.op_a_reg_tag == forwarding_reg) {
                entry.op_a_is_ready = true;
                entry.op_a_value = forwarding_value;
                entry.op_a_reg_tag = 0;
            }
            if !entry.op_b_is_ready && (entry.op_b_reg_tag == forwarding_reg) {
                entry.op_b_is_ready = true;
                entry.op_b_value = forwarding_value;
                entry.op_b_reg_tag = 0;
            }
        }
    }

    /// Pushes an integer queue entry of the given decoded instruction to the integer queue.
    fn add_integer_queue_entry(
        &mut self,
        current_state: &Processor,
        decoded_instruction: &DecodedInstruction,
    ) {
        let (physical_op_a_reg_tag, op_a_ready) =
            self.get_operand_info(decoded_instruction.op_a_reg_tag, false);
        let (physical_op_b_reg_tag, op_b_ready) = self.get_operand_info(
            decoded_instruction.op_b_reg_tag,
            decoded_instruction.immediate,
        );

        let physical_dest_register =
            self.map_destination_register(decoded_instruction.logical_destination);

        self.integer_queue.push(IntegerQueueEntry::new(
            physical_dest_register,
            op_a_ready,
            physical_op_a_reg_tag,
            current_state.physical_register_file[physical_op_a_reg_tag as usize],
            op_b_ready,
            physical_op_b_reg_tag,
            current_state.get_operand_b_value(decoded_instruction, physical_op_b_reg_tag),
            decoded_instruction.op_code.clone(),
            decoded_instruction.pc,
        ));
    }

    /// Pushes an active list entry of the given decoded instruction to the active list.
    fn add_active_list_entry(&mut self, decoded_instruction: &DecodedInstruction) {
        let old_dest_register = self.map_register(decoded_instruction.logical_destination);
        self.active_list.push(ActiveListEntry::new(
            false,
            false,
            decoded_instruction.logical_destination,
            old_dest_register,
            decoded_instruction.pc,
        ));
    }

    /// Get operand B value based on whether it is an immediate value or a register value.
    fn get_operand_b_value(
        &self,
        decoded_instruction: &DecodedInstruction,
        physical_op_b_reg_tag: u8,
    ) -> u64 {
        if decoded_instruction.immediate {
            decoded_instruction.immediate_value as u64
        } else {
            self.physical_register_file[physical_op_b_reg_tag as usize]
        }
    }

    /// Helper function to determine the physical register and readiness of an operand.
    /// If the operand is ready, the physical register tag is set to 0.
    fn get_operand_info(&self, reg_tag: u8, is_immediate: bool) -> (u8, bool) {
        // Immediate operands are always considered "ready" and don't have a physical register tag.
        if is_immediate {
            (0, true)
        } else {
            let physical_reg_tag = self.map_register(reg_tag);
            let is_ready = self.register_is_ready(physical_reg_tag);
            // If the operand is ready, we disregard the physical register tag by setting it to 0.
            let effective_reg_tag = if is_ready { 0 } else { physical_reg_tag };
            (effective_reg_tag, is_ready)
        }
    }

    /// Checks if there are enough resources to process the next four instructions.
    fn has_sufficient_resources(&self) -> bool {
        self.free_list.len() >= DECODED_BUFFER_SIZE
            && self.active_list.len() + DECODED_BUFFER_SIZE <= ACTIVE_LIST_SIZE
            && self.integer_queue.len() + DECODED_BUFFER_SIZE <= INTEGER_QUEUE_SIZE
    }

    /// Clear the decoded instructions and their PCs after processing
    fn clear_decoded_instructions(&mut self) {
        self.decoded_instructions.clear();
        self.decoded_pcs.clear();
    }

    /// Looks up a register in the register map table and returns the corresponding physical register.
    fn map_register(&self, logical_register: u8) -> u8 {
        self.register_map_table[logical_register as usize]
    }

    /// Gets the next free register from the free list.
    /// The free list is a FIFO queue.
    /// This also updates the map table with the new physical register and sets the busy bit.
    fn map_destination_register(&mut self, logical_dest: u8) -> u8 {
        let physical_dest_register = self.get_next_free_register();
        self.register_map_table[logical_dest as usize] = physical_dest_register;
        self.set_busy(physical_dest_register);
        physical_dest_register
    }

    /// Gets the next free register from the free list.
    fn get_next_free_register(&mut self) -> u8 {
        self.free_list.remove(0)
    }

    /// Checks if busy bit is set for a register.
    fn register_is_ready(&self, register: u8) -> bool {
        self.busy_bit_table[register as usize] == false
    }

    /// Sets the busy bit for a register.
    fn set_busy(&mut self, register: u8) {
        self.busy_bit_table[register as usize] = true;
    }

    /// Unsets the busy bit for a register.
    fn set_free(&mut self, register: u8) {
        self.busy_bit_table[register as usize] = false;
    }

    /// Resets execution units
    fn reset_alus(&mut self) {
        for alu in self.alus.iter_mut() {
            alu.reset();
        }
    }

    /// Resets integer queue
    fn reset_integer_queue(&mut self) {
        self.integer_queue.clear();
    }
}
