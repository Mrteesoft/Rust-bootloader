//writer.rs
mod constants;

use core::{
    fmt::{self, Write},
    ptr,
    sync::atomic::{AtomicBool, Ordering},
};

use alloc::vec;
use alloc::vec::Vec;
use bootloader_api::info::{FrameBufferInfo, PixelFormat};
use constants::font_constants;
use constants::font_constants::{BACKUP_CHAR, CHAR_RASTER_HEIGHT, CHAR_RASTER_WIDTH, FONT_WEIGHT};
use lazy_static::lazy_static;
use noto_sans_mono_bitmap::{get_raster, RasterizedChar};
use spin::Mutex;

extern crate alloc;

/// Additional vertical space between lines
const LINE_SPACING: usize = 2;

/// Additional horizontal space between characters.
const LETTER_SPACING: usize = 0;

// Tab spacing (horizontal spacing)
const TAB_SPACING: usize = 30;

/// Padding from the border. Prevent that font is too close to border.
const BORDER_PADDING: usize = 5;

/*
Overview of the additions in this file:
- Insert mode: keep a shadow grid (Vec<Option<char>>) of what's on screen so we can shift text
  right/left instead of painting over pixels. Lines redraw from this buffer to avoid artifacts.
- Visible blinking cursor: draw/erase/toggle functions paint a caret over the current cell; the
  timer interrupt flips the caret state.
- Helpers: coordinate/index helpers and redraw logic to keep cursor and buffer aligned.
*/

/// Returns the raster of the given char or the raster of [`font_constants::BACKUP_CHAR`].
pub fn get_char_raster(c: char) -> RasterizedChar {
    fn get(c: char) -> Option<RasterizedChar> {
        get_raster(c, FONT_WEIGHT, CHAR_RASTER_HEIGHT)
    }
    get(c).unwrap_or_else(|| get(BACKUP_CHAR).expect("Should get raster of backup char."))
}

/// Allows logging text to a pixel-based framebuffer.
pub struct FrameBufferWriter {
    framebuffer: &'static mut [u8],
    info: FrameBufferInfo,
    x_pos: usize,
    y_pos: usize,
    // Shadow text grid so we can shift characters for insert/backspace instead of overwriting pixels.
    cols: usize,
    rows: usize,
    buffer: Vec<Option<char>>,
}

lazy_static! {
    pub static ref FRAME_BUFFER_WRITER: Mutex<Option<FrameBufferWriter>> = Mutex::new(None);
}

static CURSOR_VISIBLE: AtomicBool = AtomicBool::new(true);

impl FrameBufferWriter {
    /// Creates a new logger that uses the given framebuffer.
    pub fn new(framebuffer: &'static mut [u8], info: FrameBufferInfo) {
        // Initialize the frame_buffer_writer if it's None
        // let frame_buffer_writer = FrameBufferWriter::new_(framebuffer, info);
        let mut logger = Self {
            framebuffer,
            info,
            x_pos: 0,
            y_pos: 0,
            cols: 0,
            rows: 0,
            buffer: Vec::new(),
        };

        logger.clear();
        let frame_buffer_writer = logger;
        if FRAME_BUFFER_WRITER.lock().is_none() {
            *FRAME_BUFFER_WRITER.lock() = Some(frame_buffer_writer);
        }
    }

    /// Creates a new logger that uses the given framebuffer.
    // pub fn new_(framebuffer: &'static mut [u8], info: FrameBufferInfo) -> Self {
    //     let mut logger = Self {
    //         framebuffer,
    //         info,
    //         x_pos: 0,
    //         y_pos: 0,
    //     };

    //     logger.clear();
    //     logger
    // }

    fn newline(&mut self) {
        self.erase_cursor();
        let line_height = font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
        let mut row = self.current_row();
        row += 1;
        if row >= self.rows {
            self.clear();
            row = 0;
        }
        self.x_pos = BORDER_PADDING;
        self.y_pos = BORDER_PADDING + row * line_height;
        self.draw_cursor();
    }

    fn carriage_return(&mut self) {
        self.erase_cursor();
        self.x_pos = BORDER_PADDING;
        self.draw_cursor();
    }

    fn width(&self) -> usize {
        self.info.width
    }

    fn height(&self) -> usize {
        self.info.height
    }

    fn tab(&mut self) {
        self.x_pos += TAB_SPACING;
    }

    pub fn set_y_pos(&mut self, y: usize) {
        self.erase_cursor();
        self.y_pos = y;
        self.draw_cursor();
    }

    pub fn set_x_pos(&mut self, x: usize) {
        self.erase_cursor();
        self.x_pos = x;
        self.draw_cursor();
    }

    pub fn cursor_left(&mut self) {
        self.erase_cursor();
        if self.x_pos > BORDER_PADDING {
            // Move the cursor back by one character width
            self.x_pos -= font_constants::CHAR_RASTER_WIDTH;
        } else {
            if self.y_pos
                >= font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING + BORDER_PADDING
            {
                self.x_pos = self.width() - (font_constants::CHAR_RASTER_WIDTH + LETTER_SPACING);
                self.y_pos -= font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
            } else {
                // Already at the top-left position, can't go back further
                return;
            }
        }
        self.draw_cursor();
    }

    pub fn cursor_right(&mut self) {
        self.erase_cursor();
        if self.x_pos + font_constants::CHAR_RASTER_WIDTH + LETTER_SPACING < self.width() {
            // Move the cursor forward by one character width
            self.x_pos += font_constants::CHAR_RASTER_WIDTH + LETTER_SPACING;
        } else {
            // Reached the end of the line, move to the next line if possible
            self.newline();
            return;
        }
        self.draw_cursor();
    }

    pub fn cursor_up(&mut self) {
        self.erase_cursor();
        if self.y_pos >= font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING + BORDER_PADDING {
            // Move the cursor up by one line height
            self.y_pos -= font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
        } else {
            // Already at the top position, can't go up further
            return;
        }
        self.draw_cursor();
    }

    pub fn cursor_down(&mut self) {
        self.erase_cursor();
        if self.y_pos + font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING < self.height() {
            // Move the cursor down by one line height
            self.y_pos += font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
        } else {
            // Reached the bottom of the screen, can't go down further
            return;
        }
        self.draw_cursor();
    }

    pub fn backspace(&mut self) {
        // Remove character before cursor and shift remainder left.
        let col = self.current_col();
        let row = self.current_row();
        if col == 0 && row == 0 {
            return;
        }
        self.erase_cursor();
        if self.x_pos > BORDER_PADDING {
            // Move the cursor back by one character width
            self.x_pos -= font_constants::CHAR_RASTER_WIDTH;
        } else {
            if self.y_pos
                >= font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING + BORDER_PADDING
            {
                self.x_pos = self.width() - (font_constants::CHAR_RASTER_WIDTH + LETTER_SPACING);
                self.y_pos -= font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
            } else {
                // Already at the top-left position, can't go back further
                return;
            }
        }
        let target_col = self.current_col();
        let target_row = self.current_row();
        let start = self.index(target_row, target_col);
        let end = self.index(target_row, self.cols - 1);
        for i in start..end {
            self.buffer[i] = self.buffer[i + 1];
        }
        self.buffer[end] = None;
        self.redraw_line(target_row);
        self.draw_cursor();
    }

    /// Erases all text on the screen. Resets `self.x_pos` and `self.y_pos`.
    pub fn clear(&mut self) {
        let char_w = font_constants::CHAR_RASTER_WIDTH + LETTER_SPACING;
        let char_h = font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
        self.cols = (self.width().saturating_sub(2 * BORDER_PADDING)) / char_w;
        self.rows = (self.height().saturating_sub(2 * BORDER_PADDING)) / char_h;
        if self.cols == 0 || self.rows == 0 {
            self.cols = 1;
            self.rows = 1;
        }
        self.buffer = vec![None; self.cols * self.rows];
        self.x_pos = BORDER_PADDING;
        self.y_pos = BORDER_PADDING;
        self.framebuffer.fill(0);
        CURSOR_VISIBLE.store(true, Ordering::SeqCst);
        self.draw_cursor();
    }

    /// Writes a single char to the framebuffer. Takes care of special control characters, such as
    /// newlines and carriage returns.
    pub fn write_char(&mut self, c: char) {
        self.erase_cursor();
        match c {
            '\t' => self.tab(),
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            c => {
                if self.current_col() + 1 >= self.cols {
                    self.newline();
                }
                if self.current_row() >= self.rows {
                    self.clear();
                }
                self.insert_into_buffer(c);
            }
        }
        self.draw_cursor();
    }

    /// Prints a rendered char into the framebuffer.
    /// Updates `self.x_pos`.
    #[allow(dead_code)]
    pub fn write_rendered_char(&mut self, rendered_char: RasterizedChar) {
        for (y, row) in rendered_char.raster().iter().enumerate() {
            for (x, byte) in row.iter().enumerate() {
                self.write_pixel(self.x_pos + x, self.y_pos + y, *byte);
            }
        }
        self.x_pos += rendered_char.width() + LETTER_SPACING;
    }
    

    fn write_pixel(&mut self, x: usize, y: usize, intensity: u8) {
        let pixel_offset = (y * self.info.stride) + x; // Added bracket
        let color = match self.info.pixel_format {
            PixelFormat::Rgb => [intensity, intensity, intensity / 2, 0],
            PixelFormat::Bgr => [intensity / 2, intensity, intensity, 0],
            PixelFormat::U8 => [if intensity > 200 { 0xf } else { 0 }, 0, 0, 0],
            other => {
                // set a supported (but invalid) pixel format before panicking to avoid a double
                // panic; it might not be readable though
                self.info.pixel_format = PixelFormat::Rgb;
                panic!("pixel format {:?} not supported in logger", other)
            }
        };
        let bytes_per_pixel = self.info.bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;
        self.framebuffer[byte_offset..(byte_offset + bytes_per_pixel)]
            .copy_from_slice(&color[..bytes_per_pixel]);
        let _ = unsafe { ptr::read_volatile(&self.framebuffer[byte_offset]) };
    }

    fn current_col(&self) -> usize {
        (self.x_pos.saturating_sub(BORDER_PADDING)) / (CHAR_RASTER_WIDTH + LETTER_SPACING)
    }

    fn current_row(&self) -> usize {
        (self.y_pos.saturating_sub(BORDER_PADDING)) / (CHAR_RASTER_HEIGHT.val() + LINE_SPACING)
    }

    fn index(&self, row: usize, col: usize) -> usize {
        row * self.cols + col
    }

    fn insert_into_buffer(&mut self, c: char) {
        let row = self.current_row();
        let col = self.current_col();
        if row >= self.rows || col >= self.cols {
            return;
        }
        // shift right within line
        let start = self.index(row, col);
        let end = self.index(row, self.cols - 1);
        for i in (start + 1..=end).rev() {
            self.buffer[i] = self.buffer[i - 1];
        }
        self.buffer[start] = Some(c);
        self.redraw_line(row);
        // move cursor one step right
        let next_col = (col + 1).min(self.cols.saturating_sub(1));
        self.x_pos = BORDER_PADDING + next_col * (CHAR_RASTER_WIDTH + LETTER_SPACING);
        // if we were at the last column, wrap to next line start
        if col + 1 >= self.cols {
            self.newline();
        }
    }

    fn redraw_line(&mut self, row: usize) {
        if row >= self.rows {
            return;
        }
        // clear line band
        let y_start = BORDER_PADDING + row * (CHAR_RASTER_HEIGHT.val() + LINE_SPACING);
        for y in 0..CHAR_RASTER_HEIGHT.val() {
            for col in 0..self.cols {
                let x_start = BORDER_PADDING + col * (CHAR_RASTER_WIDTH + LETTER_SPACING);
                for x in 0..CHAR_RASTER_WIDTH {
                    self.write_pixel(x_start + x, y_start + y, 0);
                }
            }
        }
        // redraw chars
        for col in 0..self.cols {
            if let Some(ch) = self.buffer[self.index(row, col)] {
                let x = BORDER_PADDING + col * (CHAR_RASTER_WIDTH + LETTER_SPACING);
                let y = y_start;
                self.draw_char_at(ch, x, y);
            }
        }
    }

    fn draw_char_at(&mut self, c: char, x: usize, y: usize) {
        let rendered = get_char_raster(c);
        for (dy, row) in rendered.raster().iter().enumerate() {
            for (dx, byte) in row.iter().enumerate() {
                self.write_pixel(x + dx, y + dy, *byte);
            }
        }
    }

    pub fn draw_cursor(&mut self) {
        if !CURSOR_VISIBLE.load(Ordering::SeqCst) {
            CURSOR_VISIBLE.store(true, Ordering::SeqCst);
        }
        let x_start = self.x_pos;
        let y_start = self.y_pos;
        // Fill the current cell so the caret is visible over any background glyph.
        for y in 0..CHAR_RASTER_HEIGHT.val() {
            for x in 0..CHAR_RASTER_WIDTH {
                self.write_pixel(x_start + x, y_start + y, 200);
            }
        }
    }

    pub fn erase_cursor(&mut self) {
        if CURSOR_VISIBLE.load(Ordering::SeqCst) {
            CURSOR_VISIBLE.store(false, Ordering::SeqCst);
        }
        let x_start = self.x_pos;
        let y_start = self.y_pos;
        // Clear the cell back to black, then redraw any underlying character.
        for y in 0..CHAR_RASTER_HEIGHT.val() {
            for x in 0..CHAR_RASTER_WIDTH {
                self.write_pixel(x_start + x, y_start + y, 0);
            }
        }
        // redraw character under cursor if exists
        let row = self.current_row();
        let col = self.current_col();
        if row < self.rows && col < self.cols {
            if let Some(ch) = self.buffer[self.index(row, col)] {
                self.draw_char_at(
                    ch,
                    BORDER_PADDING + col * (CHAR_RASTER_WIDTH + LETTER_SPACING),
                    BORDER_PADDING + row * (CHAR_RASTER_HEIGHT.val() + LINE_SPACING),
                );
            }
        }
    }

    pub fn toggle_cursor(&mut self) {
        if CURSOR_VISIBLE.fetch_xor(true, Ordering::SeqCst) {
            // was visible, now hide
            self.erase_cursor();
        } else {
            self.draw_cursor();
        }
    }

    #[allow(dead_code)]
    pub fn reset_cursor(&mut self) {
        CURSOR_VISIBLE.store(true, Ordering::SeqCst);
        self.draw_cursor();
    }
}

unsafe impl Send for FrameBufferWriter {}
unsafe impl Sync for FrameBufferWriter {}

impl fmt::Write for FrameBufferWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.write_char(c);
        }
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::writer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => (print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    // Use the global frame_buffer_writer
    use x86_64::instructions::interrupts;

    // To avoid deadlock, disable interrupts while the Mutex is locked
    interrupts::without_interrupts(|| {
        if let Some(frame_buffer_writer) = FRAME_BUFFER_WRITER.lock().as_mut() {
            frame_buffer_writer.write_fmt(args).unwrap();
        }
    });
}
