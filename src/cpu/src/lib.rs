
pub mod tests;
pub mod opcode;
pub mod bus;
use std::fmt::Display;

use bus::Bus;
use opcode::*;
// masks
const CARRY_MASK: u8 = 0b0000_0001;
const ZERO_MASK: u8 = 0b0000_0010;
const IRQ_MASK: u8 = 0b0000_0100;
const DECIMAL_MASK: u8 = 0b0000_1000;
const BREAK1_MASK: u8 = 0b0001_0000;
const BREAK2_MASK: u8 = 0b0001_0000;
const OVERFLOW_MASK: u8 = 0b0100_0000;
const NEGATIVE_MASK: u8 = 0b1000_0000;

// sizes
const RAM_SIZE: usize = 0xFFFF;
const STACK: usize = 0x0100;
const STACK_RESET: u8 = 0xfd;

// address
const PC_LOAD_ADDR: u16 = 0xFFFC;
const CODE_START_ADDR: usize = 0x0600;





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
   Accumulator,
}

pub trait Mem {
    fn mem_read(&self, addr: u16) -> u8;

    fn mem_write(&mut self, addr: u16, data: u8);

    fn mem_read_u16(&self, pos: u16) -> u16 {
        let lo = self.mem_read(pos) as u16;
        let hi = self.mem_read(pos + 1) as u16;
        (hi << 8) | (lo as u16)
    }

    fn mem_write_u16(&mut self, pos: u16, data: u16) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xff) as u8;
        self.mem_write(pos, lo);
        self.mem_write(pos + 1, hi);
    }
}

pub struct CPU{
    A: u8,
    X: u8,
    Y: u8,
    pc: u16,
    sp: u8,
    state: u8,
    ram: [u8; RAM_SIZE],
    bus: Bus,
}

/*==============memory============*/
impl Mem for CPU {
    fn mem_read(&self, addr: u16) -> u8 {
        self.bus.mem_read(addr)
    }

    fn mem_write(&mut self, addr: u16, data: u8) {
        self.bus.mem_write(addr, data);
    }
}
use std::io::Read;

impl CPU {
    // debug
    pub fn print_status(&mut self, code: &OpCode){

        // clear screen and move cursor
        print!("\x1b[2J\x1b[H");
        println!("INSTRUCTION:");
        match code.mode {
            AddressingMode::NoneAddressing => {
                println!("INS: {}", code.name);
            }
            _ => {
                println!("INS: {}; operand: {}", code.name, self.fetch_operand(&code.mode));
            }
        }
        println!("REGISTERS:");
        println!("A: {} X: {} Y: {} pc: {:x} sp: {:x}", self.A, self.X, self.Y, self.pc, self.sp);
        println!("STATUS:");
        println!("C: {} Z: {} I: {} B1: {} B2: {} V: {} N: {}", self.fetch_carry_bit(), self.fetch_zero_bit(), self.fetch_irq_bit(), self.fetch_break1_bit(), self.fetch_break2_bit(), self.fetch_overflow_bit(), self.fetch_negative_bit());
    }
    // constructor
    pub fn new() ->Self{
        Self { A: 0, X: 0, Y: 0, pc: CODE_START_ADDR as u16, sp: STACK_RESET, state: 0b0010_0100, ram: [0; RAM_SIZE], bus: Bus::new()}
    }

    /*==============helpers============*/
    fn set_bflag(&mut self, five: bool, six: bool){
        self.set_state(BREAK1_MASK, five);
        self.set_state(BREAK2_MASK, six);
    }
    fn fetch_carry_bit(&mut self) -> u8 {
        (self.state >> 0) & 1
    }
    fn fetch_zero_bit(&mut self) -> u8 {
        (self.state >> 1) & 1
    }
    fn fetch_irq_bit(&mut self) -> u8 {
        (self.state >> 2) & 1
    }
    fn fetch_decimal_bit(&mut self) -> u8 {
        (self.state >> 3) & 1
    }
    fn fetch_break1_bit(&mut self) -> u8 {
        (self.state >> 4) & 1
    }
    fn fetch_break2_bit(&mut self) -> u8 {
        (self.state >> 5) & 1
    }
    fn fetch_overflow_bit(&mut self) -> u8 {
        (self.state >> 6) & 1
    }
    fn fetch_negative_bit(&mut self) -> u8 {
        (self.state >> 7) & 1
    }
    fn set_reg_A(&mut self, data: u8){
        self.A = data;
        self.set_state4reg(self.A);
    }
    fn set_reg_X(&mut self, data: u8){
        self.X = data;
        self.set_state4reg(self.X);
    }
    fn set_reg_Y(&mut self, data: u8){
        self.Y = data;
        self.set_state4reg(self.Y);
    }
    fn compare(&mut self, mode: &AddressingMode, compare_with: u8) {
        let data = self.fetch_operand(mode);
        let set_carry = data <= compare_with;
        self.set_state(CARRY_MASK, set_carry);
        self.set_zero_negative(compare_with.wrapping_sub(data));
    }

    fn stack_pop(&mut self) -> u8 {
        self.sp = self.sp.wrapping_add(1);
        self.mem_read((STACK as u16) + self.sp as u16)
    }

    fn stack_pop_u16(&mut self) -> u16 {
        let lo = self.stack_pop() as u16;
        let hi = self.stack_pop() as u16;

        hi << 8 | lo
    }

    fn stack_push_u16(&mut self, data:u16) {
        let hi = (data >> 8) as u8;
        let lo = (data & 0xff) as u8;
        self.stack_push(hi);
        self.stack_push(lo);
    }

    fn stack_push(&mut self, data: u8) {
        self.mem_write((STACK as u16) + self.sp as u16, data);
        self.sp = self.sp.wrapping_sub(1)
    }

    fn branch(&mut self, condition: bool) {
        if condition {
            let jump: i8 = self.mem_read(self.pc) as i8;
            let jump_addr = self
                .pc
                .wrapping_add(1)
                .wrapping_add(jump as u16);

            self.pc = jump_addr;
        }
    }

    fn fetch_operand(&mut self, mode: &AddressingMode) -> u8{
        match mode {
            AddressingMode::Accumulator => self.A,
            _ => {
                let operand_address = self.get_operand_address(mode);
                let operand = self.mem_read(operand_address);
                operand
            }   
        }  // mode
    }  // fetch operand

    /* fetch next instruction and update pc */
    fn fetch_next(&mut self) -> u8{
        let op = self.mem_read(self.pc);
        self.pc += 1;
        op
    }

    /* set state with a state mask */
    fn set_state(&mut self, mask: u8, state: bool) {
        if state {
            self.state |= mask;
        }   else {
            self.state &= !mask;
        }
    }

    /* set zero & negative mask for one data */
    fn set_state4reg(&mut self, reg: u8){
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

    /*bit test */
    fn bit(&mut self, mode: &AddressingMode) {
        let data = self.fetch_operand(mode);
        let and = self.A & data == 0;
        self.set_state(ZERO_MASK, and);

        self.set_state(NEGATIVE_MASK, data & 0b10000000 > 0);
        self.set_state(OVERFLOW_MASK, data & 0b01000000 > 0);
    }

    // set zero and negative and carry flag, return result
    fn set_zero_negative(&mut self, result:u8){
        let seventh_bit = if (result & 0xFF) == 1 {true} else {false};
        self.set_state(NEGATIVE_MASK, seventh_bit);
        let is_zero = result == 0;
        self.set_state(ZERO_MASK, is_zero);
    }

    fn get_operand_address(&self, mode: &AddressingMode) -> u16{
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
                self.mem_read_u16(self.pc)
            }
            AddressingMode::Absolute_X => {
                let pos = self.mem_read_u16(self.pc );
                let addr = pos.wrapping_add(self.X as u16);
                addr
            }
            AddressingMode::Absolute_Y => {
                let pos = self.mem_read_u16(self.pc);
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
            _ => unimplemented!("")
        }
    }



    pub fn reset(&mut self){
        self.A = 0;
        self.X = 0;
        self.Y = 0;
        self.state = 0b0010_0100;
        self.pc = self.mem_read_u16(PC_LOAD_ADDR);
        self.sp = STACK_RESET;
    }

    pub fn load_program(&mut self, program: &Vec<u8>){
        for i in 0..(program.len() as u16) {
            self.mem_write(0x0600 + i, program[i as usize]);
        }
        self.mem_write_u16(PC_LOAD_ADDR, CODE_START_ADDR as u16);
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
    fn lda(&mut self, mode: &AddressingMode){
        // load operand into A
        let operand = self.fetch_operand(mode);
        self.set_reg_A(operand);
    }
    // store a to mem
    fn sta(&mut self, mode: &AddressingMode){
        let operand_address = self.get_operand_address(mode);
        self.mem_write(operand_address, self.A);
    }

    // load to X
    fn ldx(&mut self, mode: &AddressingMode){
        let operand = self.fetch_operand(mode);
        // load operand into X
        self.set_reg_X(operand);
    }
    // store a to mem
    fn stx(&mut self, mode: &AddressingMode){
        let operand_address = self.get_operand_address(mode);
        //println!("stx: mem is {:x}", operand_address);
        self.mem_write(operand_address, self.X);
    }

    // load to Y
    fn ldy(&mut self, mode: &AddressingMode){
        let operand = self.fetch_operand(mode);
        // load operand into Y
        self.set_reg_Y(operand);
    }
    // store a to mem
    fn sty(&mut self, mode: &AddressingMode){
        let operand_address = self.get_operand_address(mode);
        //
        
        //println!("sty: mem is {:x}", operand_address);
        self.mem_write(operand_address, self.Y);
    }

    /*arithmetic */

    fn add_to_register_a(&mut self, data: u8) {
        let sum = self.A as u16
            + data as u16
            + (if self.fetch_carry_bit() == 1{
                1
            } else {
                0
            }) as u16;

        let carry = sum > 0xff;

        if carry {
            self.set_state(CARRY_MASK, true);
        } else {
            self.set_state(CARRY_MASK, false);
        }

        let result = sum as u8;

        if (data ^ result) & (result ^ self.A) & 0x80 != 0 {
            self.set_state(OVERFLOW_MASK, true);
        } else {
            self.set_state(OVERFLOW_MASK, true);
        }

        self.set_reg_A(result);
    }

    fn adc(&mut self, mode: &AddressingMode){
        let operand = self.fetch_operand(mode);
        self.add_to_register_a(operand);
    }

    fn sbc(&mut self, mode: &AddressingMode){
        let operand = self.fetch_operand(mode);
        self.add_to_register_a(((operand as i8).wrapping_neg().wrapping_sub(1)) as u8);
    }

    fn and(&mut self, mode: &AddressingMode){
        let operand = self.fetch_operand(mode);
        self.A = self.A & operand;
        // set zero&neg bit
        self.set_state4reg(self.A);
    }

    fn xor(&mut self, mode: &AddressingMode){
        let operand = self.fetch_operand(mode);
        self.A = self.A ^ operand;
        // set zero&neg bit
        self.set_state4reg(self.A);
    }

    fn ior(&mut self, mode: &AddressingMode){
        let operand = self.fetch_operand(mode);
        self.A = self.A | operand;
        // set zero&neg bit
        self.set_state4reg(self.A);
    }

    pub fn run(&mut self){
        self.run_with_callback(|_| {});
    }

    pub fn run_with_callback<F> (&mut self, mut callback: F) where F:FnMut(&mut CPU){
        // manually set pc for now
        self.pc = 0x0600;
        let ref opcodes = *opcode::OPCODES_MAP;
        loop {
            // callback
            callback(self);

            // fetch next instruction
            let op = self.fetch_next();
            let program_counter_state = self.pc;
            // fetch code map
            let code = opcodes.get(&op).expect(&format!("OpCode {:x} is not recongnized", op));
            // for debugging
            self.print_status(code);
            // pattern matching for instruction
            match op {
                // return 
                // rti
                0x40 => {
                    self.state = self.stack_pop();
                    self.set_bflag(false, true);
                    self.pc = self.stack_pop_u16();
                }
                // rts
                0x60 => {
                    self.pc = self.stack_pop_u16() + 1;
                }
                /* JMP Absolute */
                0x4c => {
                    let mem_address = self.mem_read_u16(self.pc);
                    self.pc = mem_address;
                }

                /* JMP Indirect */
                0x6c => {
                    let mem_address = self.mem_read_u16(self.pc);
                    // let indirect_ref = self.mem_read_u16(mem_address);
                    //6502 bug mode with with page boundary:
                    //  if address $3000 contains $40, $30FF contains $80, and $3100 contains $50,
                    // the result of JMP ($30FF) will be a transfer of control to $4080 rather than $5080 as you intended
                    // i.e. the 6502 took the low byte of the address from $30FF and the high byte from $3000

                    let indirect_ref = if mem_address & 0x00FF == 0x00FF {
                        let lo = self.mem_read(mem_address);
                        let hi = self.mem_read(mem_address & 0xFF00);
                        (hi as u16) << 8 | (lo as u16)
                    } else {
                        self.mem_read_u16(mem_address)
                    };
                    self.pc = indirect_ref;
                }
                // jsr
                0x20 => {
                    let mem_address = self.mem_read_u16(self.pc);
                    self.stack_push_u16(self.pc+2-1);
                    self.pc = mem_address;
                }
                // bcc
                0x90 => {
                    let cc = self.fetch_carry_bit() == 0;
                    self.branch(cc);
                }
                // bcs
                0xb0 => {
                    let cc = self.fetch_carry_bit() == 1;
                    self.branch(cc);
                }
                // beq
                0xf0 => {
                    let cc = self.fetch_zero_bit() == 1;
                    self.branch(cc);
                }
                // bne
                0xd0 => {
                    let cc = self.fetch_zero_bit() == 0;
                    self.branch(cc);
                }
                // bpl
                0x10 => {
                    let cc = self.fetch_negative_bit() == 0;
                    self.branch(cc);
                }
                // bmi
                0x30 => {
                    let cc = self.fetch_negative_bit() == 1;
                    self.branch(cc);
                }
                // bvs
                0x70 => {
                    let cc = self.fetch_overflow_bit() == 1;
                    self.branch(cc);
                }
                // bvc
                0x50 => {
                    let cc = self.fetch_overflow_bit() == 0;
                    self.branch(cc);
                }

                // compare
                // cmp
                0x24 | 0x2c => {
                    self.bit(&code.mode);
                }
                0xc9 | 0xc5 | 0xd5 | 0xcd | 0xdd | 0xd9 | 0xc1 | 0xd1 => {
                    self.compare(&code.mode, self.A);
                }
                // cpx
                0xe0 | 0xe4 | 0xec => {
                    self.compare(&code.mode, self.X);
                }
                // cpy
                0xc0 | 0xc4 | 0xcc => {
                    self.compare(&code.mode, self.Y);
                }
                // LDA #nn: load operand into A
                0xa9 | 0xa5 | 0xb5 | 0xad | 0xbd | 0xb9 | 0xa1 | 0xb1 => {
                    self.lda(&code.mode);
                }
                // STA
                0x85 | 0x95 | 0x8d | 0x9d | 0x99 | 0x81 | 0x91 => {
                    self.sta(&code.mode);
                }
                // LDX
                0xa2 | 0xa6 | 0xb6 | 0xae | 0xbe => {
                    self.ldx(&code.mode);
                }
                // STX
                0x86 | 0x96 | 0x8e => {
                    self.stx(&code.mode);
                }
                // LDY
                0xa0 | 0xa4 | 0xb4 | 0xac | 0xbc => {
                    self.ldy(&code.mode);
                }
                // STY
                0x84 | 0x94 | 0x8c => {
                    self.sty(&code.mode);
                }
                // TAX: transfer X to A
                0xaa => {
                    self.X = self.A;
                    self.set_state4reg(self.X);
                }
                // TAY
                0xa8 => {
                    self.Y = self.A;
                    self.set_state4reg(self.Y);
                }
                // TXA
                0x8a => {
                    self.A = self.X;
                    self.set_state4reg(self.A);
                }
                // TYA
                0x98 => {
                    self.A = self.Y;
                    self.set_state4reg(self.A);
                }
                // TSX
                0xba => {
                    self.X = self.sp;
                    self.set_state4reg(self.X);
                }
                // TXS
                0x9a => {
                    self.sp = self.X;
                }

                /*arithmetic */
                // lsr acc
                0x4a => {
                    let result = self.A >> 1;
                    // set flags
                    self.set_zero_negative(result);
                    let last_bit = if (self.A & 1) == 1 {true}  else {false};
                    self.set_state(CARRY_MASK, last_bit);
                    self.A = result;
                }
                // lsr
                0x46 | 0x56 | 0x4e | 0x5e => {
                    let addr = self.get_operand_address(&code.mode);
                    let operand = self.mem_read(addr);
                    let result = operand >> 1;
                    self.mem_write(addr, result);
                    // set flags
                    let last_bit = if (operand & 1) == 1 {true}  else {false};
                    self.set_state(CARRY_MASK, last_bit);
                    self.set_zero_negative(result);
                }
                // asl acc
                0x0a => {
                    let result = self.A << 1;
                    // set flags
                    self.set_zero_negative(result);
                    let last_bit = if (self.A >> 7) == 1 {true}  else {false};
                    self.set_state(CARRY_MASK, last_bit);
                    self.A = result;
                }
                // asl
                0x06 | 0x16 | 0x0e | 0x1e => {
                    let addr = self.get_operand_address(&code.mode);
                    let operand = self.mem_read(addr);
                    let result = operand << 1;
                    self.mem_write(addr, result);
                    // set flags
                    let last_bit = if (operand >> 7) == 1 {true}  else {false};
                    self.set_state(CARRY_MASK, last_bit);
                    self.set_zero_negative(result);
                }
                // rol acc
                0x2a => {
                    let mut data = self.A;
                    let old_carry = self.fetch_carry_bit() == 1;
                    let set_carry = data >> 7 == 1 ;
                    self.set_state(CARRY_MASK, set_carry);
                    data = data << 1;
                    if old_carry {
                        data = data | 1;
                    }
                    self.set_reg_A(data);
                }
                // rol
                0x26 | 0x36 | 0x2e | 0x3e => {
                    let addr = self.get_operand_address(&code.mode);
                    let mut data = self.mem_read(addr);
                    let old_carry = self.fetch_carry_bit() == 1;
                    let set_carry = data >> 7 == 1 ;
                    self.set_state(CARRY_MASK, set_carry);
                    data = data << 1;
                    if old_carry {
                        data = data | 1;
                    }
                    self.mem_write(addr, data);
                    self.set_zero_negative(data);
                }
                // ror acc
                0x6a => {
                    let mut data = self.A;
                    let old_carry = self.fetch_carry_bit() == 1;
                    let set_carry = data & 1 == 1 ;
                    self.set_state(CARRY_MASK, set_carry);
                    data = data << 1;
                    if old_carry {
                        data = data | 0b1000_0000;
                    }
                    self.set_reg_A(data);
                }
                // ror
                0x66 | 0x76 | 0x6e | 0x7e => {
                    let addr = self.get_operand_address(&code.mode);
                    let mut data = self.mem_read(addr);
                    let old_carry = self.fetch_carry_bit() == 1;
                    let set_carry = data & 1 == 1 ;
                    self.set_state(CARRY_MASK, set_carry);
                    data = data << 1;
                    if old_carry {
                        data = data | 0b1000_0000;
                    }
                    self.mem_write(addr, data);
                    self.set_zero_negative(data);
                }
                // and 
                0x29 | 0x25 | 0x35 | 0x2d | 0x3d | 0x39 | 0x21 | 0x31 => {
                    self.and(&code.mode);
                }
                // eor 
                0x49 | 0x45 | 0x55 | 0x4d | 0x5d | 0x59 | 0x41 | 0x51 => {
                    self.xor(&code.mode);
                }
                // ora 
                0x09 | 0x05 | 0x15 | 0x0d | 0x1d | 0x19 | 0x01 | 0x11 => {
                    self.ior(&code.mode);
                }
                // adc 
                0x69 | 0x65 | 0x75 | 0x6d | 0x7d | 0x79 | 0x61 | 0x71 => {
                    self.adc(&code.mode);
                }
                // sbc 
                0xe9 | 0xe5 | 0xf5 | 0xed | 0xfd | 0xf9 | 0xe1 | 0xf1 => {
                    self.sbc(&code.mode);
                }
                // dec
                0xc6 | 0xd6 | 0xce | 0xde => {
                    let addr = self.get_operand_address(&code.mode);
                    let mut data = self.mem_read(addr);
                    data = data.wrapping_sub(1);
                    self.mem_write(addr, data);
                    self.set_zero_negative(data);
                }
                // inc
                0xe6 | 0xf6 | 0xee | 0xfe => {
                    let addr = self.get_operand_address(&code.mode);
                    let mut data = self.mem_read(addr);
                    data = data.wrapping_add(1);
                    self.mem_write(addr, data);
                    self.set_zero_negative(data);
                }
                // dex
                0xca => {
                    self.X = self.X.wrapping_sub(1);
                    self.set_state4reg(self.X);
                }
                // dey
                0x88 => {
                    self.Y = self.Y.wrapping_sub(1);
                    self.set_state4reg(self.Y);
                }
                // iny
                0xc8 => {
                    self.Y = self.Y.wrapping_add(1);
                    self.set_state4reg(self.Y);
                }
                /* push&pop */
                // php
                0x08 => {
                    self.stack_push(self.state);
                    self.set_bflag(true, true);
                }
                // plp
                0x28 => {
                    self.state = self.stack_pop();
                    self.set_bflag(false, true);
                }
                // pha
                0x48 => {
                    self.stack_push(self.A);
                }
                // pla
                0x68 => {
                    self.A = self.stack_pop();
                    self.set_state4reg(self.A);
                }

                /* set flags */
                // sec
                0x38 => {
                    self.set_state(CARRY_MASK, true);
                }
                0x18 => {
                    self.set_state(CARRY_MASK, false);
                }
                // sed
                0xf8 => {
                    self.set_state(DECIMAL_MASK, true);
                }
                0xd8 => {
                    self.set_state(DECIMAL_MASK, false);
                }
                // sei
                0x78 => {
                    self.set_state(IRQ_MASK, true);
                }
                0x58 => {
                    self.set_state(IRQ_MASK, false);
                }
                // clv
                0xb8 => {
                    self.set_state(OVERFLOW_MASK, false);
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
                //nop
                0xea => {}
                _ => unimplemented!("op code {}", op)
            }   // match
            if program_counter_state == self.pc {
                self.pc += (code.bytes - 1) as u16;
            }
        }   // cpu loop
    }

}




