
pub mod tests;
pub mod opcode;
use opcode::*;
// masks
const CARRY_MASK: u8 = 0b0000_0001;
const ZERO_MASK: u8 = 0b0000_0010;
const IRQ_MASK: u8 = 0b0000_0100;
const DECIMAL_MASK: u8 = 0b0000_1000;
const BREAK_MASK: u8 = 0b0001_0000;
const OVERFLOW_MASK: u8 = 0b0100_0000;
const NEGATIVE_MASK: u8 = 0b1000_0000;

// sizes
const RAM_SIZE: usize = 0xFFFF;

// address
const PC_LOAD_ADDR: u16 = 0xFFFC;
const CODE_START_ADDR: usize = 0x8000;

#[derive(Debug)]
#[allow(non_camel_case_types)]
pub enum AddressingMode {
   Immediate,
   ZeroPage,
   ZeroPage_X,
   ZeroPage_Y,
   Absolute,
   Absolute_X,
   Absolute_Y,
   Indirect_X,
   Indirect_Y,
   NoneAddressing,
}

pub struct CPU{
    A: u8,
    X: u8,
    Y: u8,
    pc: u16,
    sp: u16,
    state: u8,
    ram: [u8; RAM_SIZE],
}

impl CPU {
    // constructor
    pub fn new() ->Self{
        Self { A: 0, X: 0, Y: 0, pc: CODE_START_ADDR as u16, sp: 0, state: 0, ram: [0; RAM_SIZE], }
    }

    /*==============helpers============*/
    pub fn fetch_next(&mut self) -> u8{
        let op = self.ram[self.pc as usize];
        self.pc += 1;
        op
    }

    pub fn set_state(&mut self, mask: u8, state: bool) {
        if state {
            self.state |= mask;
        }   else {
            self.state &= !mask;
        }
    }

    pub fn set_state4reg(&mut self, reg: u8){
        // set state if zero
        if reg == 0 {
            self.set_state(ZERO_MASK, true);
        }   else {
            self.set_state(ZERO_MASK, false);
        }
        // set state if negative
        if reg & NEGATIVE_MASK != 0 {
            self.set_state(NEGATIVE_MASK, true);
        }   else {
            self.set_state(NEGATIVE_MASK, false);
        }
    }

    pub fn get_operand_address(&self, mode: &AddressingMode) -> u16{
        match mode {
            AddressingMode::Immediate => {
                self.pc
            }
            AddressingMode::ZeroPage => {
                self.mem_read(self.pc) as u16
            }
            AddressingMode::ZeroPage_X => {
                let pos = self.mem_read(self.pc) as u16;
                let addr = pos.wrapping_add(self.X as u16);
                addr
            }
            AddressingMode::ZeroPage_Y => {
                let pos = self.mem_read(self.pc) as u16;
                let addr = pos.wrapping_add(self.Y as u16);
                addr
            }
            AddressingMode::Absolute => {
                self.mem_read_16(self.pc)
            }
            AddressingMode::Absolute_X => {
                let pos = self.mem_read_16(self.pc );
                let addr = pos.wrapping_add(self.X as u16);
                addr
            }
            AddressingMode::Absolute_Y => {
                let pos = self.mem_read_16(self.pc);
                let addr = pos.wrapping_add(self.Y as u16);
                addr
            }
            AddressingMode::Indirect_X => {
                let base = self.mem_read(self.pc);

               let ptr: u8 = (base as u8).wrapping_add(self.X);
               let lo = self.mem_read(ptr as u16);
               let hi = self.mem_read(ptr.wrapping_add(1) as u16);
               (hi as u16) << 8 | (lo as u16)
            }
            AddressingMode::Indirect_Y => {
                let base = self.mem_read(self.pc);
 
                let lo = self.mem_read(base as u16);
                let hi = self.mem_read((base as u8).wrapping_add(1) as u16);
                let deref_base = (hi as u16) << 8 | (lo as u16);
                let deref = deref_base.wrapping_add(self.Y as u16);
                deref
            }
            AddressingMode::NoneAddressing => unimplemented!("")
        }
    }

    /*==============memory============*/
    pub fn mem_read(&self, addr: u16) -> u8{
        self.ram[addr as usize]
    }

    pub fn mem_write(&mut self, addr: u16, value: u8) {
        self.ram[addr as usize] = value;
    }

    pub fn mem_read_16(&self, addr: u16) -> u16{
        let lo = self.mem_read(addr) as u16;
        let hi = self.mem_read(addr+1) as u16;
        (hi << 8) | (lo as u16)
    }

    pub fn mem_write_16(&mut self, addr: u16, value: u16){
        let lo = (value & 0xFF) as u8;
        let hi = (value >> 8) as u8;
        self.mem_write(addr, lo);
        self.mem_write(addr, hi);
    }

    pub fn reset(&mut self){
        self.A = 0;
        self.X = 0;
        self.Y = 0;
        self.state = 0;
        self.pc = self.mem_read_16(PC_LOAD_ADDR);
    }

    pub fn load_program(&mut self, program: &Vec<u8>){
        self.ram[CODE_START_ADDR..(CODE_START_ADDR+program.len())].copy_from_slice(&program[..]);
        self.mem_write_16(PC_LOAD_ADDR, CODE_START_ADDR as u16);
    }

    pub fn load_and_run(&mut self, program: Vec<u8>){
        self.load_program(&program);
        self.run();
    }

    pub fn load_run_reset(&mut self, program: Vec<u8>){
        self.load_program(&program);
        self.reset();
        self.run();
    }


    /*==============cpu loop============*/
    // load to a
    pub fn lda(&mut self, mode: &AddressingMode){
        let operand_address = self.get_operand_address(mode);
        let operand = self.mem_read(operand_address);
        // load operand into A
        self.A = operand;
        self.set_state4reg(self.A);
    }
    // store a to mem
    pub fn sta(&mut self, mode: &AddressingMode){
        let operand_address = self.get_operand_address(mode);
        self.mem_write(operand_address, self.A);
    }



    pub fn run(&mut self){
        let ref opcodes = *opcode::OPCODES_MAP;
        loop {
            let op = self.fetch_next();
            let program_counter_state = self.pc;
            let code = opcodes.get(&op).expect(&format!("OpCode {:x} is not recongnized", op));
            match op {
                // LDA #nn: load operand into A
                0xa9 | 0xa5 | 0xb5 | 0xad | 0xbd | 0xb9 | 0xa1 | 0xb1 => {
                    self.lda(&code.mode);
                }
                // STA
                0x85 | 0x95 | 0x8d | 0x9d | 0x99 | 0x81 | 0x91 => {
                    self.sta(&code.mode);
                }
                // TAX: transfer X to A
                0xaa => {
                    self.X = self.A;
                    self.set_state4reg(self.X);
                }
                // INX: increment X
                0xe8 => {
                    self.X = self.X.wrapping_add(1);
                    self.set_state4reg(self.X);
                }
                // BRK
                0x00 => {
                    return;
                }
                _ => unimplemented!("op code {}", op)
            }   // match
            if program_counter_state == self.pc {
                self.pc += (code.bytes - 1) as u16;
            }
        }   // cpu loop
    }   // interpret
}




