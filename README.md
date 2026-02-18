CSC308 Mini OS

What this is
A tiny Rust kernel that boots with bootloader_api. We added two user-visible features so it behaves like a simple text editor: a blinking cursor and true insert mode.

What we changed
- Blinking cursor: the PIT timer flips the cursor on/off so you always see where the next character will land.
- Insert mode with a shadow text buffer: typing in the middle pushes characters to the right; backspace pulls them left; lines are redrawn cleanly.

How to build
- Run: cargo build
- Outputs land in: 	arget/debug/build/os_with_bootloader-*/out/ 

How to run
- Easiest: cargo run (defaults to UEFI; flip in os_with_bootloader/src/main.rs if you need BIOS).
- Manual BIOS run (PowerShell/WSL-friendly):
`
 = (Get-ChildItem -Recurse target\debug\build -Filter bios.img | Sort-Object LastWriteTime -Descending | Select-Object -First 1).FullName
qemu-system-x86_64 -drive format=raw,file="" -serial stdio -vga std
`

Quick test script
1) Type ABC
2) Left Arrow twice (cursor before B)
3) Type X → you should see AXBC (insert mode working)
4) Backspace → ABC (no pixel junk)
5) Watch the cursor blink; move with arrows and it follows.

Notes / edge cases
- If a line is completely full, shifting may drop the final character of that line (simple drop policy).
- Scrolling past the last row clears the screen (same as the starter code).
