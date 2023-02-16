

use crate::cartridge::Mirroring;
use crate::ppu_registers::{AddrRegister, ControlRegister, PPURegister, MaskRegister, StatusRegister, ScrollRegister};

const  MAX_CYCLE:usize = 314;
const MAX_SCAN_LINE:usize = 261;

pub struct PPU{
    chr_rom: Vec<u8>,   // visuals of a game stored on a cartridge
    palette_table: [u8; 32],    // internal memory to keep palette tables used by a screen
    vram: [u8; 2048],    // 2 KiB banks of space to hold background information
    oam_data: [u8; 256], // internal memory to keep state of sprites, OAM => Object Attribute Memory

    mirroring: Mirroring,

    internal_data_buf: u8, // internal buffer behavior for RAM and ROM: read [0x2007] in CPU will return this data

    // Clock cycles and scan lines
    pub clock_cycles: usize,
    pub scan_lines: usize,
    nmi_irq: Option<u8>,
    


    // 8 ppu registers
    // reg_ctrl: 0x2000 - instructs PPU on general logic flow (which memory table to use, if PPU should interrupt CPU, etc.)
    // reg_mask: 0x2001 - instructs PPU how to render sprites and background
    // reg_status: 0x2002 -  reporting PPU status
    // reg_oam_addr: 0x2003 - responsible for sprites' address
    // reg_oam_data: 0x2004 - responsible for sprites' data
    // reg_scroll: 0x2005 - instructs PPU how to set a viewport
    // reg_addr&reg_data: 0x2006&0x2007 - provide access to the memory map available for PPU
    reg_addr: AddrRegister,
    reg_ctrl: ControlRegister,
    reg_oam_addr: u8,
    reg_mask: MaskRegister,
    reg_status: StatusRegister,
    reg_scroll: ScrollRegister,

}



impl PPU{
    pub fn new_empty_rom() -> Self {
        PPU::new(vec![0; 2048], Mirroring::HORIZONTAL)
    }

    pub fn new(chr_rom: Vec<u8>, mirroring: Mirroring) -> Self {
        PPU{
            chr_rom: chr_rom,
            mirroring: mirroring,
            vram: [0; 2048],
            oam_data: [0; 64 * 4],
            palette_table: [0; 32],
            internal_data_buf: 0,
            clock_cycles: 0,
            scan_lines: 0,
            nmi_irq: None,
            reg_addr: AddrRegister::new(),
            reg_ctrl:ControlRegister::new(),
            reg_oam_addr: 0,
            reg_mask: MaskRegister::new(),
            reg_status: StatusRegister::new(),
            reg_scroll: ScrollRegister::new(),
        }
    }

    pub fn pull_nmi_irq(&mut self) -> Option<u8>{
        // take irq and leave num_irq to None
        self.nmi_irq.take()
    }


    // write to registers
    pub fn write_to_ppu_mask(&mut self, value: u8){
        self.reg_mask.update(value)
    }

    pub fn write_to_ppu_addr(&mut self, value: u8) {
        self.reg_addr.update(value);
    }

    pub fn write_to_ctrl(&mut self, value: u8){
        let before_ctrl_nmi = self.reg_ctrl.generate_vblank_nmi();
        self.reg_ctrl.update(value);
        if !before_ctrl_nmi && self.reg_ctrl.generate_vblank_nmi() && self.reg_status.is_in_vblank(){
            self.nmi_irq = Some(1);
        }
    }

    pub fn write_to_oam_addr(&mut self, value: u8){
        self.reg_oam_addr = value;
    }

    pub fn write_to_oam_data(&mut self, value: u8){
        self.oam_data[self.reg_oam_addr as usize] = value;
        self.reg_oam_addr = self.reg_oam_addr.wrapping_add(1);
    }

    pub fn write_to_scroll(&mut self, value: u8){
        self.reg_scroll.update(value);
    }

    pub fn write_to_data(&mut self, value: u8){
        let addr = self.reg_addr.get();
        self.increment_vram_addr();

        match addr {
            0..=0x1fff => panic!("rom space cannot be wite, requested = {}", addr),
            0x2000..=0x2fff => {
                let vram_addr = self.mirror_vram_addr(addr);
                self.vram[(vram_addr as usize)] = value;
            }
            0x3000..=0x3eff => panic!("addr space 0x3000..0x3eff is not expected to be used, requested = {} ", addr),
            //Addresses $3F10/$3F14/$3F18/$3F1C are mirrors of $3F00/$3F04/$3F08/$3F0C
            0x3f10 | 0x3f14 | 0x3f18 | 0x3f1c => {
                let add_mirror = addr - 0x10;
                self.palette_table[(add_mirror - 0x3f00) as usize] = value;
            }
            0x3f00..=0x3fff =>{
                self.palette_table[(addr-0x3f00) as usize] = value;
            }
            _ => panic!("unexpected access to mirrored space {}", addr),
        }
    }

    pub fn write_oam_dma(&mut self, data: &[u8]) {
        assert_eq!(data.len(), 256);
        for x in data.iter() {
            self.oam_data[self.reg_oam_addr as usize] = *x;
            self.reg_oam_addr = self.reg_oam_addr.wrapping_add(1);
        }
    }

    // reading PPU memory
    fn increment_vram_addr(&mut self){
        self.reg_addr.increment(self.reg_ctrl.vram_addr_increment());
    }

    pub fn read_ppu_status(&mut self) -> u8{
        let res = self.reg_status.snapshot();
        self.reg_addr.reset_latch();
        self.reg_scroll.reset_latch();
        self.reg_status.reset_vblank_status();
        res
    }

    pub fn read_oam_data(&mut self) -> u8{
        let result = self.oam_data[self.reg_oam_addr as usize];
        self.reg_oam_addr = self.reg_oam_addr.wrapping_add(1);
        result
    }

    pub fn read_data(&mut self) -> u8{
        let addr = self.reg_addr.get();
        self.increment_vram_addr();

        match addr {
            0..=0x1fff => {
                let result = self.internal_data_buf;
                self.internal_data_buf = self.chr_rom[addr as usize];
                result
            }
            0x2000..=0x2fff => {
                let result = self.internal_data_buf;
                let vram_addr = self.mirror_vram_addr(addr);
                self.internal_data_buf = self.vram[(vram_addr) as usize];
                result
            }
            0x3000..=0x3eff => panic!("addr space 0x3000..0x3eff is not expected to be used, requested = {} ", addr),
            //Addresses $3F10/$3F14/$3F18/$3F1C are mirrors of $3F00/$3F04/$3F08/$3F0C
            0x3f10 | 0x3f14 | 0x3f18 | 0x3f1c => {
                let add_mirror = addr - 0x10;
                self.palette_table[(add_mirror - 0x3f00) as usize]
            }
            0x3f00..=0x3fff =>{
                self.palette_table[(addr-0x3f00) as usize]
            }
            _ => panic!("unexpected access to mirrored space {}", addr),
        }
    }

    

    // Horizontal:
   //   [ A ] [ a ]
   //   [ B ] [ b ]
 
   // Vertical:
   //   [ A ] [ B ]
   //   [ a ] [ b ]
   // want to mao a -> A; map b -> B
   pub fn mirror_vram_addr(&self, addr: u16) -> u16 {
        let mirrored_vram = addr & 0b10111111111111; // mirror down 0x3000-0x3eff to 0x2000 - 0x2eff
        let vram_index = mirrored_vram - 0x2000; // to vram vector
        let name_table = vram_index / 0x400; // to the name table index

        match (&self.mirroring, name_table){
            (Mirroring::VERTICAL, 2) | (Mirroring::VERTICAL, 3) => vram_index - 0x800,
            (Mirroring::HORIZONTAL, 1) => vram_index - 0x400,
            (Mirroring::HORIZONTAL, 2) => vram_index - 0x400,
            (Mirroring::HORIZONTAL, 3) => vram_index - 0x800,
            _ => vram_index
        }
   }


   // Main execution logic
   pub fn tick(&mut self, cycles: usize){
        self.clock_cycles += cycles;
        if self.clock_cycles >= MAX_CYCLE {
            self.scan_lines += 1;
            self.clock_cycles -= MAX_CYCLE;
        }

        
        // match self.clock_cycles {
        //     // Cycles 1-256
        //     // The data for each tile is fetched during this phase. Each memory access takes 2 PPU cycles to complete
        //     1..=256 => {
        //         //todo!("fetch data for bg");
        //     }
        //     //Cycles 257-320
        //     // The tile data for the sprites on the next scanline are fetched here. Again, each memory access takes 2 PPU cycles to complete, and 4 are performed for each of the 8 sprites:
        //     257..=320 => {
        //         //todo!("fetch data for sprites")
        //     }
        //     321..=326 => {
        //         // fetch
        //     }
        //     _ => {

        //     }
        // }
        if self.scan_lines == 241{
            
            // generate irq
            if self.reg_ctrl.generate_vblank_nmi(){
                self.reg_status.set_vblank_status(true);
                self.nmi_irq = Some(1);
            }
        }

        if self.scan_lines > MAX_SCAN_LINE{
            self.scan_lines = 0;
            self.reg_status.reset_vblank_status();

        }


   }
}

#[cfg(test)]
pub mod test {
    use super::*;

    #[test]
    fn test_ppu_vram_writes() {
        let mut ppu = PPU::new_empty_rom();
        ppu.write_to_ppu_addr(0x23);
        ppu.write_to_ppu_addr(0x05);
        ppu.write_to_data(0x66);

        assert_eq!(ppu.vram[0x0305], 0x66);
    }

    #[test]
    fn test_ppu_vram_reads() {
        let mut ppu = PPU::new_empty_rom();
        ppu.write_to_ctrl(0);
        ppu.vram[0x0305] = 0x66;

        ppu.write_to_ppu_addr(0x23);
        ppu.write_to_ppu_addr(0x05);

        ppu.read_data(); //load_into_buffer
        assert_eq!(ppu.reg_addr.get(), 0x2306);
        assert_eq!(ppu.read_data(), 0x66);
    }

    #[test]
    fn test_ppu_vram_reads_cross_page() {
        let mut ppu = PPU::new_empty_rom();
        ppu.write_to_ctrl(0);
        ppu.vram[0x01ff] = 0x66;
        ppu.vram[0x0200] = 0x77;
        
        ppu.write_to_ppu_addr(0x21);
        
        ppu.write_to_ppu_addr(0xff);
        println!("reg addr is {:x}", ppu.reg_addr.get());
        ppu.read_data(); //load_into_buffer
        println!("reg addr is {:x}", ppu.reg_addr.get());
        assert_eq!(ppu.read_data(), 0x66);
        println!("reg addr is {:x}", ppu.reg_addr.get());
        assert_eq!(ppu.read_data(), 0x77);
        println!("reg addr is {:x}", ppu.reg_addr.get());
    }

    #[test]
    fn test_ppu_vram_reads_step_32() {
        let mut ppu = PPU::new_empty_rom();
        ppu.write_to_ctrl(0b100);
        ppu.vram[0x01ff] = 0x66;
        ppu.vram[0x01ff + 32] = 0x77;
        ppu.vram[0x01ff + 64] = 0x88;

        ppu.write_to_ppu_addr(0x21);
        ppu.write_to_ppu_addr(0xff);

        ppu.read_data(); //load_into_buffer
        assert_eq!(ppu.read_data(), 0x66);
        assert_eq!(ppu.read_data(), 0x77);
        assert_eq!(ppu.read_data(), 0x88);
    }

    // Horizontal: https://wiki.nesdev.com/w/index.php/Mirroring
    //   [0x2000 A ] [0x2400 a ]
    //   [0x2800 B ] [0x2C00 b ]
    #[test]
    fn test_vram_horizontal_mirror() {
        let mut ppu = PPU::new_empty_rom();
        ppu.write_to_ppu_addr(0x24);
        ppu.write_to_ppu_addr(0x05);

        ppu.write_to_data(0x66); //write to a

        ppu.write_to_ppu_addr(0x28);
        ppu.write_to_ppu_addr(0x05);

        ppu.write_to_data(0x77); //write to B

        ppu.write_to_ppu_addr(0x20);
        ppu.write_to_ppu_addr(0x05);

        ppu.read_data(); //load into buffer
        assert_eq!(ppu.read_data(), 0x66); //read from A

        ppu.write_to_ppu_addr(0x2C);
        ppu.write_to_ppu_addr(0x05);

        ppu.read_data(); //load into buffer
        assert_eq!(ppu.read_data(), 0x77); //read from b
    }

    // Vertical: https://wiki.nesdev.com/w/index.php/Mirroring
    //   [0x2000 A ] [0x2400 B ]
    //   [0x2800 a ] [0x2C00 b ]
    #[test]
    fn test_vram_vertical_mirror() {
        let mut ppu = PPU::new(vec![0; 2048], Mirroring::VERTICAL);

        ppu.write_to_ppu_addr(0x20);
        ppu.write_to_ppu_addr(0x05);

        ppu.write_to_data(0x66); //write to A

        ppu.write_to_ppu_addr(0x2C);
        ppu.write_to_ppu_addr(0x05);

        ppu.write_to_data(0x77); //write to b

        ppu.write_to_ppu_addr(0x28);
        ppu.write_to_ppu_addr(0x05);

        ppu.read_data(); //load into buffer
        assert_eq!(ppu.read_data(), 0x66); //read from a

        ppu.write_to_ppu_addr(0x24);
        ppu.write_to_ppu_addr(0x05);

        ppu.read_data(); //load into buffer
        assert_eq!(ppu.read_data(), 0x77); //read from B
    }

    #[test]
    fn test_read_status_resets_latch() {
        let mut ppu = PPU::new_empty_rom();
        ppu.vram[0x0305] = 0x66;

        ppu.write_to_ppu_addr(0x21);
        ppu.write_to_ppu_addr(0x23);
        ppu.write_to_ppu_addr(0x05);

        println!("reg addr is {:x}", ppu.reg_addr.get());
        ppu.read_data(); //load_into_buffer
        assert_ne!(ppu.read_data(), 0x66);
        println!("reg addr is {:x}", ppu.reg_addr.get());
        ppu.read_ppu_status();

        ppu.write_to_ppu_addr(0x23);
        ppu.write_to_ppu_addr(0x05);
        println!("reg addr is {:x}", ppu.reg_addr.get());
        ppu.read_data(); //load_into_buffer
        println!("reg addr is {:x}", ppu.reg_addr.get());
        assert_eq!(ppu.read_data(), 0x66);
    }

    #[test]
    fn test_ppu_vram_mirroring() {
        let mut ppu = PPU::new_empty_rom();
        ppu.write_to_ctrl(0);
        ppu.vram[0x0305] = 0x66;

        ppu.write_to_ppu_addr(0x63); //0x6305 -> 0x2305
        ppu.write_to_ppu_addr(0x05);

        ppu.read_data(); //load into_buffer
        assert_eq!(ppu.read_data(), 0x66);
        // assert_eq!(ppu.addr.read(), 0x0306)
    }

    #[test]
    fn test_read_status_resets_vblank() {
        let mut ppu = PPU::new_empty_rom();
        ppu.reg_status.set_vblank_status(true);

        let status = ppu.read_ppu_status();

        assert_eq!(status >> 7, 1);
        assert_eq!(ppu.reg_status.snapshot() >> 7, 0);
    }

    #[test]
    fn test_oam_read_write() {
        let mut ppu = PPU::new_empty_rom();
        ppu.write_to_oam_addr(0x10);
        ppu.write_to_oam_data(0x66);
        ppu.write_to_oam_data(0x77);

        ppu.write_to_oam_addr(0x10);
        assert_eq!(ppu.read_oam_data(), 0x66);

        ppu.write_to_oam_addr(0x11);
        assert_eq!(ppu.read_oam_data(), 0x77);
    }

    #[test]
    fn test_oam_dma() {
        let mut ppu = PPU::new_empty_rom();

        let mut data = [0x66; 256];
        data[0] = 0x77;
        data[255] = 0x88;

        ppu.write_to_oam_addr(0x10);
        ppu.write_oam_dma(&data);

        ppu.write_to_oam_addr(0xf); //wrap around
        assert_eq!(ppu.read_oam_data(), 0x88);

        ppu.write_to_oam_addr(0x10);
        assert_eq!(ppu.read_oam_data(), 0x77);
  
        ppu.write_to_oam_addr(0x11);
        assert_eq!(ppu.read_oam_data(), 0x66);
    }
}