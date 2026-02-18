use crate::println;
use lazy_static::lazy_static;
use pc_keyboard::KeyCode;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use pic8259::ChainedPics;
use spin::Mutex;
use x86_64::structures::idt::InterruptDescriptorTable;
use x86_64::structures::idt::InterruptStackFrame;
use x86_64::instructions::port::Port;
use crate::writer::FRAME_BUFFER_WRITER;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: Mutex<ChainedPics> =
    Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

// Blink state
const CURSOR_BLINK_DIVISOR: u64 = 20;
static BLINK_TICKS: Mutex<u64> = Mutex::new(0);


lazy_static! {
    static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> = Mutex::new(
        Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore)
    );
}


extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n Stack Frame:\n {:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n Stack Frame:\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn general_protection_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) {
    println!(
        "EXCEPTION: GENERAL PROTECTION\n Error Code: {:#?}\n Stack Frame:\n{:#?}",
        _error_code, stack_frame
    );
}

extern "x86-interrupt" fn invalid_opcode_handler(stack_frame: InterruptStackFrame) {
    println!(
        "EXCEPTION: INVALID OPCODE\n Stack Frame:\n {:#?}",
        stack_frame
    );
}

// Handling Timer Interrupts

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    {
        let mut ticks = BLINK_TICKS.lock();
        *ticks += 1;
        if *ticks % CURSOR_BLINK_DIVISOR == 0 {
            // Drive cursor blink from PIT ticks (cursor paint/erase lives in writer.rs).
            x86_64::instructions::interrupts::without_interrupts(|| {
                if let Some(writer) = FRAME_BUFFER_WRITER.lock().as_mut() {
                    writer.toggle_cursor();
                }
            });
        }
    }

    // End of Interrupt(EOI)
    // PIC expects an explicit “end of interrupt” (EOI) signal from our interrupt handler.
    // This signal tells the controller that the interrupt was processed and that the system is ready to receive the next interrupt.
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }

    // The `notify_end_of_interrupt` figures out whether the primary or secondary PIC sent the interrupt
    // and then uses the command and data ports to send an EOI signal to the respective controllers.
    // If the secondary PIC sent the interrupt, both PICs need to be notified because the secondary PIC
    // is connected to an input line of the primary PIC.
}

//

// Handling Keyboard Interrupts

// We now see that a `k` appears on the screen when we press a key. However, this only works for
// the first key we press. Even if we continue to press keys, no more k's appear on the screen.
// This is because the keyboard controller won’t send another interrupt until we have read the
// so-called scancode of the pressed key.
extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // print!("k");

    // Reading the Scancodes

    // To find out which key was pressed, we need to query the keyboard controller.
    // We do this by reading from the data port of the PS/2 controller,
    // which is the I/O port with the number 0x60:
    

    // We use the Port type of the x86_64 crate to read a byte from the keyboard’s data port.
    // This byte is called the scancode and it represents the key press/release.
    // We don’t do anything with the scancode yet, other than print it to the screen
    // let mut port = Port::new(0x60);
    // let scancode: u8 = unsafe { port.read() };



    let mut keyboard= KEYBOARD.lock();
    let mut port = Port::new(0x60);

    let scancode: u8 = unsafe { port.read() };// Interrupt happens here

    
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => {
                    x86_64::instructions::interrupts::without_interrupts(|| {
                        if let Some(frame_buffer_writer) = FRAME_BUFFER_WRITER.lock().as_mut() {
                            if character == '\u{0008}' {
                                frame_buffer_writer.backspace();
                            } else {
                                frame_buffer_writer.write_char(character);
                            }
                        }
                    });
                }

                DecodedKey::RawKey(key) => {
                    x86_64::instructions::interrupts::without_interrupts(|| {
                        if let Some(frame_buffer_writer) = FRAME_BUFFER_WRITER.lock().as_mut() {
                            if key == KeyCode::ArrowLeft {
                                frame_buffer_writer.cursor_left();
                            } else if key == KeyCode::ArrowRight {
                                frame_buffer_writer.cursor_right();
                            } else if key == KeyCode::ArrowUp {
                                frame_buffer_writer.cursor_up();
                            } else if key == KeyCode::ArrowDown {
                                frame_buffer_writer.cursor_down();
                            } 
                            // else {
                            //     // frame_buffer_writer.write_char(key.unicode_char().unwrap_or('?'));
                            //     print!("{:?}", key);
                            // }
                        }
                    })
                },
            }
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}


lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.double_fault.set_handler_fn(double_fault_handler);
        idt.general_protection_fault.set_handler_fn(general_protection_handler);
        idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);
        idt[InterruptIndex::Timer.as_usize()].set_handler_fn(timer_interrupt_handler); // Timer Interrupt
        idt[InterruptIndex::Keyboard.as_usize()].set_handler_fn(keyboard_interrupt_handler); // Keyboard interrupt

        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

pub fn init() {
    init_idt();
    unsafe {
        // PICS.lock().initialize();
        let mut pics = PICS.lock();
        pics.initialize();

        pics.write_masks(0b1111_1100, 0b1111_1111);
    }
    x86_64::instructions::interrupts::enable(); 
}

// #[macro_export]
// macro_rules! input_char {
//     () => {
//         {
//             use pc_keyboard::DecodedKey;
//             use x86_64::instructions::port::Port;

//             let mut keyboard = crate::KEYBOARD.lock();
//             let mut port = Port::new(0x60);

//             let scancode: u8 = unsafe { port.read() };
//             if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
//                 if let Some(key) = keyboard.process_keyevent(key_event) {
//                     match key {
//                         DecodedKey::Unicode(character) => {
//                             if character == '\u{0008}' {
//                                 x86_64::instructions::interrupts::without_interrupts(|| {
//                                     if let Some(frame_buffer_writer) = crate::FRAME_BUFFER_WRITER.lock().as_mut() {
//                                         frame_buffer_writer.backspace();
//                                     }
//                                 });
//                             } else {
//                                 crate::print!("{}", character);
//                                 Some(character)
//                             }
//                         }
//                         DecodedKey::RawKey(_) => None,
//                     }
//                 }
//             }
//         }
//     };
// }
