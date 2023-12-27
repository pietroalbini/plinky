use std::fmt::Formatter;

const BYTES_PER_COLUMN: usize = 16;

pub(crate) fn render_hex(f: &mut Formatter<'_>, prefix: &str, bytes: &[u8]) -> std::fmt::Result {
    f.write_fmt(format_args!("\n{prefix}     "))?;
    let columns = bytes.len().min(BYTES_PER_COLUMN); // Between 0 and 16
    for column in 0..columns {
        f.write_fmt(format_args!("?{column:x} "))?;
    }
    f.write_fmt(format_args!("  (len: {:#x})", bytes.len()))?;
    f.write_fmt(format_args!("\n{prefix}   +-"))?;
    for _ in 0..columns {
        f.write_str("---")?;
    }
    f.write_str("+")?;

    let mut multiline = false;
    let mut pending_as_ascii = Vec::new();
    let mut row = 0;
    for byte in bytes {
        if pending_as_ascii.is_empty() {
            f.write_str("\n")?;
            f.write_str(prefix)?;
            f.write_fmt(format_args!("{row:x}? | "))?;
            row += 1;
        } else {
            f.write_str(" ")?;
        }
        f.write_fmt(format_args!("{byte:0>2x}"))?;
        pending_as_ascii.push(*byte);

        if pending_as_ascii.len() >= BYTES_PER_COLUMN {
            render_ascii(f, &mut pending_as_ascii)?;
            multiline = true;
        }
    }

    // Handle remaining bytes by filling in whitespace and rendering that.
    if !pending_as_ascii.is_empty() {
        if multiline {
            f.write_str(&"   ".repeat(BYTES_PER_COLUMN - pending_as_ascii.len()))?;
        }
        render_ascii(f, &mut pending_as_ascii)?;
    }
    f.write_str("\n")?;

    Ok(())
}

fn render_ascii(f: &mut Formatter<'_>, remaining: &mut Vec<u8>) -> std::fmt::Result {
    f.write_str(" | ")?;
    for byte in remaining.drain(..) {
        if byte.is_ascii_alphanumeric() || byte.is_ascii_punctuation() || byte == b' ' {
            f.write_str(std::str::from_utf8(&[byte]).unwrap())?;
        } else {
            f.write_str(".")?;
        }
    }
    Ok(())
}
