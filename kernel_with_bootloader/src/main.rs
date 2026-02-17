// main.rs
#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

// use core::fmt::{Write, Arguments};

use good_memory_allocator::SpinLockedAllocator;
// use lazy_static::lazy_static;
// use spin::Mutex;
use bootloader_api::{
    config::Mapping,
    info::{MemoryRegion, MemoryRegionKind},
};
// use writer::FrameBufferWriter;
use x86_64::instructions::hlt;
extern crate alloc;

mod writer;
use writer::{FrameBufferWriter, FRAME_BUFFER_WRITER};

mod interrupts;
// use interrupts::{init_idt, PICS};
use interrupts::{init};

#[global_allocator]
static ALLOCATOR: SpinLockedAllocator = SpinLockedAllocator::empty();

//Use the entry_point macro to register the entry point function: bootloader_api::entry_point!(kernel_main)
//my_entry_point can be any name.
//optionally pass a custom config
pub static BOOTLOADER_CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();

    config.mappings.physical_memory = Some(Mapping::Dynamic);

    config.kernel_stack_size = 100 * 1024; // 100 KiB

    config
};
bootloader_api::entry_point!(my_entry_point, config = &BOOTLOADER_CONFIG);

// lazy_static! {
//     static ref FRAME_BUFFER_WRITER: Mutex<Option<FrameBufferWriter>> = Mutex::new(None);
// }

// fn my_entry_point(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
//     //boot_info.framebuffer is our target for display

//     let frame_buffer_info = boot_info.framebuffer.as_mut().unwrap().info();
//     let buffer = boot_info.framebuffer.as_mut().unwrap().buffer_mut();

//     let mut frame_buffer_writer = FrameBufferWriter::new(buffer, frame_buffer_info);

//     //write!(frame_buffer_writer, "Testing testing {}\n and {}", 1, 4.0 / 2.0).unwrap();
//     print!("Hello");
//     frame_buffer_writer.update_x_pos(30usize);
//     print!("World");

//     loop {
//         hlt(); //stop x86_64 from being unnecessarily busy while looping
//     }
// }

fn my_entry_point(boot_info: &'static mut bootloader_api::BootInfo) -> ! {
    let frame_buffer_info = boot_info.framebuffer.as_mut().unwrap().info();
    let buffer = boot_info.framebuffer.as_mut().unwrap().buffer_mut();
    //Let's examine our memory
    //Go through memory regions passed and add usable ones to our global allocator
    //let memory_regions_count = boot_info.memory_regions.iter().size_hint();
    //println!("{}", memory_regions_count.0);

    //Let's get the usable memory
    let last_memory_region = boot_info.memory_regions.last().unwrap();
    //println!("{:X}", last_memory_region.end);

    // println!("Memory regions:");

    //get the first bootload memory
    let mut boot_loader_memory_region = MemoryRegion::empty();

    for memory_region in boot_info.memory_regions.iter() {
        match memory_region.kind {
            MemoryRegionKind::Bootloader => {
                boot_loader_memory_region = *memory_region;
                break;
            }
            _ => continue,
        }
    }
    // println!("{:X} {:X} {:?}", boot_loader_memory_region.start, boot_loader_memory_region.end, boot_loader_memory_region.kind);

    let physical_memory_offset = boot_info.physical_memory_offset.into_option().unwrap();
    //let heap_start = 0x3E000 + physical_memory_offset;
    //let heap_size = 0x7FC2000;
    let heap_start = boot_loader_memory_region.end + 0x1 + physical_memory_offset;
    let heap_size = last_memory_region.end - (boot_loader_memory_region.end + 0x1);

    //println!("{:X} {:X}", heap_start as usize, heap_size as usize);

    unsafe {
        ALLOCATOR.init(heap_start as usize, heap_size as usize);
    }

    // Now that the heap allocator is initialized, create the framebuffer writer
    FrameBufferWriter::new(buffer, frame_buffer_info);

    // Set initial cursor position
    if let Some(writer) = FRAME_BUFFER_WRITER.lock().as_mut() {
        writer.set_x_pos(40usize);
        writer.set_y_pos(30usize);
    }

    // Initialize interrupts after writer is ready
    init();

    // use alloc::boxed::Box;

    // let x = Box::new(33);

    // writeln!(frame_buffer_writer, "Value in X is {}", x).unwrap();
    // println!("Value in X is {}", x);

    // let mut counter = 0 as u8;
    // for memory_region in boot_info.memory_regions.iter() {
    //     counter += 1;
    //     // frame_buffer_writer
    //     //     .write_fmt(format_args!("{}. ", counter)) //All other formatting macros (format!, write, println!, etc) are proxied through this one. format_args!, unlike its derived macros, avoids heap allocations.
    //     //     .unwrap();
    //     print!("{}. ", counter);
    //     // frame_buffer_writer
    //     //     .write_fmt(format_args!("{:X} ", memory_region.start)) //All other formatting macros (format!, write, println!, etc) are proxied through this one. format_args!, unlike its derived macros, avoids heap allocations.
    //     //     .unwrap();
    //     print!("{:X}. ", memory_region.start);
    //     // frame_buffer_writer
    //     //     .write_fmt(format_args!("{:X}, ", memory_region.end))
    //     //     .unwrap();
    //     print!("{:X}. ", memory_region.end);
    //     // frame_buffer_writer
    //     //     .write_fmt(format_args!(
    //     //         "size = {:X}, ",
    //     //         memory_region.end - memory_region.start
    //     //     ))
    //     //     .unwrap();
    //     print!("size = {:X}, ", memory_region.end - memory_region.start);
    //     print!("");

    //     // Use the global frame_buffer_writer
        //if let Some(writer) = FRAME_BUFFER_WRITER.lock().as_mut() {
        // match memory_region.kind {
        //     MemoryRegionKind::Usable => print!("Usable"), //write!(writer, "Usable;  ").unwrap(),
        //     MemoryRegionKind::Bootloader => print!("Bootload"), //write!(FRAME_BUFFER_WRITER, "Bootload;").unwrap(),
        //     MemoryRegionKind::UnknownUefi(_) => print!("UnkownUefi"),//{
        //     //     write!(FRAME_BUFFER_WRITER, "UnknownUefi;").unwrap();
        //     // }
        //     MemoryRegionKind::UnknownBios(_) => print!("Unknown"),//{
        //     //     write!(FRAME_BUFFER_WRITER, "UnknownBios;").unwrap();
        //     // }
        //     _ => print!("UnknownBios"),//write!(frame_buffer_writer, "UnknownBios;").unwrap(),
        // }
        
    // let mut write_fmt = |args: Arguments| {
    //     if let Some(writer) = FRAME_BUFFER_WRITER.lock().as_mut() {
    //         writer.write_fmt(args).unwrap();
    //     }
    //     // frame_buffer_writer.write_fmt(args).unwrap();
    // };

    // init_idt();
    // invoke a breakpoint exception
    // x86_64::instructions::interrupts::int3();
    

    loop {
        hlt(); //stop x86_64 from being unnecessarily busy while looping
    }
}
    

//We need to handle interrupts
// use x86_64::structures::idt::InterruptDescriptorTable;
// use x86_64::structures::idt::InterruptStackFrame;

// extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
//     // println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
// }

// extern "x86-interrupt" fn double_fault_handler(
//     stack_frame: InterruptStackFrame,
//     _error_code: u64,
// ) -> ! {
//     panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
// }

// use lazy_static::lazy_static;

// lazy_static! {
//     static ref IDT: InterruptDescriptorTable = {
//         let mut idt = InterruptDescriptorTable::new();
//         idt.breakpoint.set_handler_fn(breakpoint_handler);
//         idt.double_fault.set_handler_fn(double_fault_handler);
//         idt
//     };
// }

// /*pub fn init_idt() {
//  IDT.load();
// }*/

// pub fn init_idt() {
//     //init_idt();
//     IDT.load();
// }

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        hlt();
    }
}
