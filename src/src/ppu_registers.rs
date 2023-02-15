
// Data, oam registers are directly emulated by PPU
pub trait PPURegister {
    
    fn update(&mut self, data:u8);

}

// Scroll Register 0x2005

pub struct ScrollRegister{
    pub x: u8,
    pub y: u8,
    pub latch: bool,
}

impl PPURegister for ScrollRegister{
    fn update(&mut self, data:u8) {
        if self.latch{
            self.x= data;
        }   else {
            self.y = data;
        }
        self.latch = !self.latch;
    }
}

impl ScrollRegister {

    pub fn new()->Self{
        ScrollRegister { x: 0, y: 0, latch: true }
    }

    pub fn reset_latch(&mut self){
        self.latch = true;
    }
}

// Address Register 0x2006
pub struct AddrRegister{
    pub val: (u8, u8),  // val.0 for high; val.1 for low
    pub hi_ptr: bool,
}

impl PPURegister for AddrRegister {
    fn update(&mut self, data:u8){
        if self.hi_ptr{
            // update high part if needed
            self.val.0 = data;
        }   else {
            self.val.1 = data;
        }
        self.check_mirror();
        self.hi_ptr = !self.hi_ptr;
    }
}

impl AddrRegister{
    pub fn new() -> Self{
        AddrRegister{
            val: (0,0),
            hi_ptr: true,
        }
    }

    pub fn set(&mut self, data: u16){
        self.val.0 = (data >> 8) as u8;
        self.val.1 = (data & 0xff) as u8;
    }


    pub fn increment(&mut self, inc:u8){
        let lo = self.val.1;
        self.val.1 = self.val.1.wrapping_add(inc);

        if lo > self.val.1 {   // if carry bit (0xFF -> 0x01)
            self.val.0 = self.val.0.wrapping_add(1);
        }
        self.check_mirror();
    }

    pub fn check_mirror(&mut self){
        if self.get() > 0x3fff { //mirror down addr above 0x3fff
            self.set(self.get() & 0x3fff);
        }
    }

    pub fn get(&self) -> u16{
        (self.val.0 as u16) << 8 | (self.val.1 as u16)
    }

    pub fn reset_latch(&mut self){
        self.hi_ptr = true;
    }
}


// Control Registers 0x2000

bitflags! {

    // 7  bit  0
    // ---- ----
    // VPHB SINN
    // |||| ||||
    // |||| ||++- Base nametable address
    // |||| ||    (0 = $2000; 1 = $2400; 2 = $2800; 3 = $2C00)
    // |||| |+--- VRAM address increment per CPU read/write of PPUDATA
    // |||| |     (0: add 1, going across; 1: add 32, going down)
    // |||| +---- Sprite pattern table address for 8x8 sprites
    // ||||       (0: $0000; 1: $1000; ignored in 8x16 mode)
    // |||+------ Background pattern table address (0: $0000; 1: $1000)
    // ||+------- Sprite size (0: 8x8 pixels; 1: 8x16 pixels)
    // |+-------- PPU master/slave select
    // |          (0: read backdrop from EXT pins; 1: output color on EXT pins)
    // +--------- Generate an NMI at the start of the
    //            vertical blanking interval (0: off; 1: on)
    pub struct ControlRegister: u8 {
        const NAMETABLE1              = 0b00000001;
        const NAMETABLE2              = 0b00000010;
        const VRAM_ADD_INCREMENT      = 0b00000100;
        const SPRITE_PATTERN_ADDR     = 0b00001000;
        const BACKROUND_PATTERN_ADDR  = 0b00010000;
        const SPRITE_SIZE             = 0b00100000;
        const MASTER_SLAVE_SELECT     = 0b01000000;
        const GENERATE_NMI            = 0b10000000;
    }
 }


 impl PPURegister for ControlRegister {
    fn update(&mut self, data: u8){
        self.bits = data;
    }
}

 impl ControlRegister {
     pub fn new() -> Self {
        ControlRegister::from_bits_truncate(0b0000_0000)
     }

     pub fn vram_addr_increment(&self) -> u8{
        if !self.contains(ControlRegister::VRAM_ADD_INCREMENT) {
            1
        }   else {
            32
        }
     }

     
 }

// Mask Register 0x2001
bitflags! {
     
    //  7  bit  0
    // ---- ----
    // BGRs bMmG
    // |||| ||||
    // |||| |||+- Greyscale (0: normal color, 1: produce a greyscale display)
    // |||| ||+-- 1: Show background in leftmost 8 pixels of screen, 0: Hide
    // |||| |+--- 1: Show sprites in leftmost 8 pixels of screen, 0: Hide
    // |||| +---- 1: Show background
    // |||+------ 1: Show sprites
    // ||+------- Emphasize red (green on PAL/Dendy)
    // |+-------- Emphasize green (red on PAL/Dendy)
    // +--------- Emphasize blue
    pub struct MaskRegister: u8{
        const GREYSCALE               = 0b00000001;
        const LEFTMOST_8PXL_BACKGROUND  = 0b00000010;
        const LEFTMOST_8PXL_SPRITE      = 0b00000100;
        const SHOW_BACKGROUND         = 0b00001000;
        const SHOW_SPRITES            = 0b00010000;
        const EMPHASISE_RED           = 0b00100000;
        const EMPHASISE_GREEN         = 0b01000000;
        const EMPHASISE_BLUE          = 0b10000000;
    }

}

pub enum Color {
    Red,
    Green,
    Blue,
}

impl PPURegister for MaskRegister{
    fn update(&mut self, data:u8) {
        self.bits = data;
    }
}

impl MaskRegister {
    pub fn new() -> Self{
        MaskRegister::from_bits_truncate(0b0000_0000)
    }

    pub fn is_greyscale(&self) -> bool{
        self.contains(MaskRegister::GREYSCALE)
    }

    pub fn is_leftmost_8pxl_bg(&self) -> bool{
        self.contains(MaskRegister::LEFTMOST_8PXL_BACKGROUND)
    }

    pub fn is_leftmost_8pxl_sprite(&self) -> bool{
        self.contains(MaskRegister::LEFTMOST_8PXL_SPRITE)
    }

    pub fn is_leftmost_show_bg(&self) -> bool{
        self.contains(MaskRegister::SHOW_BACKGROUND)
    }

    pub fn is_leftmost_show_sprite(&self) -> bool{
        self.contains(MaskRegister::SHOW_SPRITES)
    }

    pub fn emphasis(&self) -> Vec<Color>{
        let mut res = Vec::<Color>::new();
        if self.contains(MaskRegister::EMPHASISE_RED){
            res.push(Color::Red);
        }
        if self.contains(MaskRegister::EMPHASISE_BLUE){
            res.push(Color::Blue);
        }
        if self.contains(MaskRegister::EMPHASISE_GREEN){
            res.push(Color::Green);
        }
        res
    }


}

// Status Register 0x2002
bitflags! {

    // 7  bit  0
    // ---- ----
    // VSO. ....
    // |||| ||||
    // |||+-++++- Least significant bits previously written into a PPU register
    // |||        (due to register not being updated for this address)
    // ||+------- Sprite overflow. The intent was for this flag to be set
    // ||         whenever more than eight sprites appear on a scanline, but a
    // ||         hardware bug causes the actual behavior to be more complicated
    // ||         and generate false positives as well as false negatives; see
    // ||         PPU sprite evaluation. This flag is set during sprite
    // ||         evaluation and cleared at dot 1 (the second dot) of the
    // ||         pre-render line.
    // |+-------- Sprite 0 Hit.  Set when a nonzero pixel of sprite 0 overlaps
    // |          a nonzero background pixel; cleared at dot 1 of the pre-render
    // |          line.  Used for raster timing.
    // +--------- Vertical blank has started (0: not in vblank; 1: in vblank).
    //            Set at dot 1 of line 241 (the line *after* the post-render
    //            line); cleared after reading $2002 and at dot 1 of the
    //            pre-render line.
    pub struct StatusRegister: u8 {
        const NOTUSED          = 0b00000001;
        const NOTUSED2         = 0b00000010;
        const NOTUSED3         = 0b00000100;
        const NOTUSED4         = 0b00001000;
        const NOTUSED5         = 0b00010000;
        const SPRITE_OVERFLOW  = 0b00100000;
        const SPRITE_ZERO_HIT  = 0b01000000;
        const VBLANK_STARTED   = 0b10000000;
    }
}

impl StatusRegister {
    pub fn new() -> Self {
        StatusRegister::from_bits_truncate(0b00000000)
    }

    pub fn set_vblank_status(&mut self, status: bool) {
        self.set(StatusRegister::VBLANK_STARTED, status);
    }

    pub fn set_sprite_zero_hit(&mut self, status: bool) {
        self.set(StatusRegister::SPRITE_ZERO_HIT, status);
    }

    pub fn set_sprite_overflow(&mut self, status: bool) {
        self.set(StatusRegister::SPRITE_OVERFLOW, status);
    }

    pub fn reset_vblank_status(&mut self) {
        self.remove(StatusRegister::VBLANK_STARTED);
    }

    pub fn is_in_vblank(&self) -> bool {
        self.contains(StatusRegister::VBLANK_STARTED)
    }

    pub fn snapshot(&self) -> u8 {
        self.bits
    }
}

// 