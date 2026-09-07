#![no_std]

use core::fmt::{self, Write};

struct Buffer<'a> {
    bytes: &'a mut [u8],
    length: usize,
}

impl Write for Buffer<'_> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        let end = self.length.checked_add(text.len()).ok_or(fmt::Error)?;
        self.bytes.get_mut(self.length..end).ok_or(fmt::Error)?.copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}

#[no_mangle]
pub unsafe extern "C" fn shin_format_float(value: f64, bytes: *mut u8, capacity: usize) -> usize {
    let mut output = Buffer { bytes: core::slice::from_raw_parts_mut(bytes, capacity), length: 0 };
    let result = if value.is_nan() { output.write_str("nan") } else { write!(output, "{value}") };
    if result.is_err() { return 0; }
    if value.is_finite() && !output.bytes[..output.length].contains(&b'.') {
        if output.write_str(".0").is_err() { return 0; }
    }
    output.length
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    extern "C" { fn abort() -> !; }
    unsafe { abort() }
}
