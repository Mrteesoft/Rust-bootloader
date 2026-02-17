CSC308 Submission Notes

What to turn in
- Make one zip: Surname_CSC308.zip
- Put only the .rs files you changed from kernel_with_bootloader/src/ (likely main.rs, interrupts.rs, writer.rs)
- Leave out target/ and any build outputs
- You can include this README if you want

What we added
- Blinking cursor (timer-driven) so you can see where the next character will land
- Insert mode with a shadow text buffer: typing in the middle shifts text right; backspace shifts left; lines redraw cleanly

Heads-up / limits
- If a line is completely full, shifting may drop the last char on that line
- When we scroll past the last row, the screen clears (same basic behavior as the starter code)
