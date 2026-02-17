CSC308 Mini OS – What Was Implemented

Summary
We extended the starter kernel with two missing features:
1) Blinking cursor so you can see the insertion point.
2) Insert mode so typing in the middle shifts text instead of overwriting pixels.

Where the code lives
- writer.rs
  - Holds FrameBufferWriter, the text renderer.
  - Added a shadow text buffer (Vec<Option<char>>) sized to the screen.
  - write_char now inserts into the buffer and shifts the rest of the line right.
  - backspace shifts left; redraw_line repaints from the buffer so pixels stay clean.
  - draw_cursor / erase_cursor / toggle_cursor paint or clear a visible caret.
- interrupts.rs
  - timer_interrupt_handler toggles the cursor every CURSOR_BLINK_DIVISOR PIT ticks, making it blink.
- main.rs (kernel_with_bootloader)
  - FrameBufferWriter::new is called after the heap allocator is initialized (it allocates the shadow buffer).

How it works (quick walkthrough)
- Insert mode: When you type at the cursor, the character is placed into the shadow buffer, the rest of the line shifts right, and the line is redrawn to the framebuffer. Backspace shifts left and redraws.
- Cursor: The timer interrupt flips a flag; writer.rs draws or erases the caret at (x_pos, y_pos). Moving with arrow keys updates x_pos/y_pos and the caret follows.

How to build
- From repo root: cargo build
  - Outputs: target/debug/build/os_with_bootloader-*/out/bios.img and uefi.img

How to run
- Recommended: cargo run (defaults to UEFI; change in os_with_bootloader/src/main.rs if needed)
- Manual BIOS example (PowerShell/WSL-compatible):
`
 = (Get-ChildItem -Recurse target\debug\build -Filter bios.img | Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
qemu-system-x86_64 -drive format=raw,file="" -serial stdio -vga std
`

What to test
- Cursor blinks and tracks arrow keys.
- Insert mode: type "HELLO WORLD", move left 5, type "X" → "HELLO XWORLD".
- Backspace in the middle pulls text left without leaving artifacts.

Known limitations
- If a line is completely full, the simplest shift policy drops the last character of that line.
- When scrolling past the last row, the screen clears (same as starter behavior).

Submission checklist
- Zip name: Surname_CSC308.zip
- Include only modified .rs files from kernel_with_bootloader/src/ (e.g., main.rs, interrupts.rs, writer.rs)
- Exclude target/ and build outputs
- You may include this README.txt if desired
