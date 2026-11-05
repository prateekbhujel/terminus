//! Terminus Prompts — Interactive terminal UI prompts in pure Rust
//! Beautiful, interactive terminal UI prompts with zero heavy dependencies.

use std::fmt::Display;
use std::io::{self, BufRead, Write};

/// ANSI color and style escape codes
pub mod ansi {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const RED: &str = "\x1b[31m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const CYAN: &str = "\x1b[36m";
    pub const WHITE: &str = "\x1b[37m";
    pub const GRAY: &str = "\x1b[90m";
}

/// Ask the user for plain text input
pub fn text(
    label: &str,
    default: Option<&str>,
    placeholder: Option<&str>,
    required: bool,
) -> io::Result<String> {
    let mut stdout = io::stdout();
    let stdin = io::stdin();

    loop {
        // Render Prompt
        print!("{}[?]{} {}{}{}", ansi::CYAN, ansi::RESET, ansi::BOLD, label, ansi::RESET);

        if let Some(d) = default {
            print!(" {}[{}]", ansi::GRAY, d);
        } else if let Some(p) = placeholder {
            print!(" {}({})", ansi::GRAY, p);
        }
        print!("{}: ", ansi::RESET);
        stdout.flush()?;

        let mut input = String::new();
        stdin.lock().read_line(&mut input)?;
        let trimmed = input.trim();

        if trimmed.is_empty() {
            if let Some(d) = default {
                println!("  {}[OK]{} {}{}{}", ansi::GREEN, ansi::RESET, ansi::GRAY, d, ansi::RESET);
                return Ok(d.to_string());
            } else if required {
                println!("  {}[ERROR] Required field. Please enter a value.{}", ansi::RED, ansi::RESET);
                continue;
            } else {
                return Ok(String::new());
            }
        }

        println!("  {}[OK]{} {}{}{}", ansi::GREEN, ansi::RESET, ansi::GRAY, trimmed, ansi::RESET);
        return Ok(trimmed.to_string());
    }
}

/// Prompt for a hidden or masked password
pub fn password(label: &str, required: bool) -> io::Result<String> {
    let mut stdout = io::stdout();
    let stdin = io::stdin();

    loop {
        print!("{}[?]{} {}{}{}: ", ansi::CYAN, ansi::RESET, ansi::BOLD, label, ansi::RESET);
        stdout.flush()?;

        let mut input = String::new();
        stdin.lock().read_line(&mut input)?;
        let trimmed = input.trim();

        if trimmed.is_empty() && required {
            println!("  {}[ERROR] Password cannot be empty.{}", ansi::RED, ansi::RESET);
            continue;
        }

        let masked = "*".repeat(trimmed.len());
        println!("  {}[OK]{} {}{}{}", ansi::GREEN, ansi::RESET, ansi::GRAY, masked, ansi::RESET);
        return Ok(trimmed.to_string());
    }
}

/// Ask for a boolean confirmation
pub fn confirm(label: &str, default: bool) -> io::Result<bool> {
    let mut stdout = io::stdout();
    let stdin = io::stdin();

    let hint = if default { "[Y/n]" } else { "[y/N]" };
    print!("{}[?]{} {}{}{} {}{}{}: ", ansi::CYAN, ansi::RESET, ansi::BOLD, label, ansi::RESET, ansi::GRAY, hint, ansi::RESET);
    stdout.flush()?;

    let mut input = String::new();
    stdin.lock().read_line(&mut input)?;
    let trimmed = input.trim().to_lowercase();

    let result = if trimmed.is_empty() {
        default
    } else { trimmed == "y" || trimmed == "yes" || trimmed == "1" || trimmed == "true" };

    let display_str = if result { "yes" } else { "no" };
    println!("  {}[OK]{} {}{}{}", ansi::GREEN, ansi::RESET, ansi::GRAY, display_str, ansi::RESET);
    Ok(result)
}

/// Select a single option from a list
pub fn select<'a, T: Display>(
    label: &str,
    options: &'a [(&'a str, T)],
    default_index: usize,
) -> io::Result<&'a str> {
    if options.is_empty() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "Options list cannot be empty"));
    }

    let mut stdout = io::stdout();
    let stdin = io::stdin();

    println!("{}[?]{} {}{}{}", ansi::CYAN, ansi::RESET, ansi::BOLD, label, ansi::RESET);
    for (i, (key, text)) in options.iter().enumerate() {
        let is_default = i == default_index;
        let marker = if is_default {
            format!("{}>{} ", ansi::GREEN, ansi::RESET)
        } else {
            "  ".to_string()
        };
        let num = format!("{}[{}]{}", ansi::GRAY, i + 1, ansi::RESET);
        println!("  {}{} {} {}({}){}", marker, num, text, ansi::DIM, key, ansi::RESET);
    }

    loop {
        print!("  Enter choice {}[1-{}, default: {}]{}: ", ansi::GRAY, options.len(), default_index + 1, ansi::RESET);
        stdout.flush()?;

        let mut input = String::new();
        stdin.lock().read_line(&mut input)?;
        let trimmed = input.trim();

        if trimmed.is_empty() {
            let (key, text) = &options[default_index];
            println!("  {}[OK]{} Selected: {}{}{} ({})", ansi::GREEN, ansi::RESET, ansi::BOLD, text, ansi::RESET, key);
            return Ok(key);
        }

        if let Ok(idx) = trimmed.parse::<usize>() {
            if idx >= 1 && idx <= options.len() {
                let (key, text) = &options[idx - 1];
                println!("  {}[OK]{} Selected: {}{}{} ({})", ansi::GREEN, ansi::RESET, ansi::BOLD, text, ansi::RESET, key);
                return Ok(key);
            }
        }

        // Also allow matching by key directly
        if let Some((key, text)) = options.iter().find(|(k, _)| k.eq_ignore_ascii_case(trimmed)) {
            println!("  {}[OK]{} Selected: {}{}{} ({})", ansi::GREEN, ansi::RESET, ansi::BOLD, text, ansi::RESET, key);
            return Ok(key);
        }

        println!("  {}[ERROR] Invalid selection. Please enter 1-{} or the option key.{}", ansi::RED, options.len(), ansi::RESET);
    }
}

/// Select multiple options from a list
pub fn multiselect<'a, T: Display>(
    label: &str,
    options: &'a [(&'a str, T)],
    defaults: &[&str],
) -> io::Result<Vec<&'a str>> {
    let mut stdout = io::stdout();
    let stdin = io::stdin();

    println!("{}[?]{} {}{}{}", ansi::CYAN, ansi::RESET, ansi::BOLD, label, ansi::RESET);
    println!("  {}Enter comma-separated numbers (e.g. 1, 3) or press Enter for defaults:{}", ansi::DIM, ansi::RESET);

    for (i, (key, text)) in options.iter().enumerate() {
        let is_selected = defaults.contains(key);
        let check = if is_selected {
            format!("{}[x]{}", ansi::GREEN, ansi::RESET)
        } else {
            format!("{}[ ]{}", ansi::GRAY, ansi::RESET)
        };
        println!("  {} {} {}. {} {}({}){}", check, ansi::GRAY, i + 1, text, ansi::DIM, key, ansi::RESET);
    }

    print!("  Choices: ");
    stdout.flush()?;

    let mut input = String::new();
    stdin.lock().read_line(&mut input)?;
    let trimmed = input.trim();

    if trimmed.is_empty() {
        let selected: Vec<&'a str> = options
            .iter()
            .filter(|(k, _)| defaults.contains(k))
            .map(|(k, _)| *k)
            .collect();
        println!("  {}[OK]{} Selected: {:?}", ansi::GREEN, ansi::RESET, selected);
        return Ok(selected);
    }

    let mut selected = Vec::new();
    for part in trimmed.split(',') {
        let p = part.trim();
        if let Ok(idx) = p.parse::<usize>() {
            if idx >= 1 && idx <= options.len() {
                selected.push(options[idx - 1].0);
            }
        } else if let Some((k, _)) = options.iter().find(|(k, _)| k.eq_ignore_ascii_case(p)) {
            selected.push(*k);
        }
    }

    println!("  {}[OK]{} Selected: {:?}", ansi::GREEN, ansi::RESET, selected);
    Ok(selected)
}

/// Execute a closure with a spinner indicator
pub fn spin<R, F: FnOnce() -> R>(message: &str, task: F) -> R {
    let mut stdout = io::stdout();
    print!("{}[*]{} {}...", ansi::MAGENTA, ansi::RESET, message);
    let _ = stdout.flush();

    let res = task();

    // Clear line and print checkmark
    print!("\r\x1b[2K");
    println!("{}[OK]{} {}{}{}", ansi::GREEN, ansi::RESET, ansi::BOLD, message, ansi::RESET);
    let _ = stdout.flush();

    res
}

/// Render an ASCII/Unicode table in the terminal
pub fn table(headers: &[&str], rows: &[Vec<String>]) {
    if headers.is_empty() {
        return;
    }

    let col_count = headers.len();
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();

    for row in rows {
        for (i, cell) in row.iter().enumerate().take(col_count) {
            widths[i] = widths[i].max(cell.len());
        }
    }

    // Top border
    let sep: String = widths.iter().map(|w| "-".repeat(w + 2)).collect::<Vec<_>>().join("+");
    println!("+{}+", sep);

    // Header row
    let mut header_line = String::new();
    for (i, h) in headers.iter().enumerate() {
        header_line.push_str(&format!(" {:<width$} |", h, width = widths[i]));
    }
    println!("|{}", header_line);
    println!("+{}+", sep);

    // Data rows
    for row in rows {
        let mut row_line = String::new();
        for (i, width) in widths.iter().enumerate().take(col_count) {
            let cell = row.get(i).map(|s| s.as_str()).unwrap_or("");
            row_line.push_str(&format!(" {:<width$} |", cell, width = width));
        }
        println!("|{}", row_line);
    }

    // Bottom border
    println!("+{}+", sep);
}

/// Print an information callout banner
pub fn info(message: impl AsRef<str>) {
    println!("{}[INFO]{} {}", ansi::CYAN, ansi::RESET, message.as_ref());
}

/// Print a warning callout banner
pub fn warning(message: impl AsRef<str>) {
    println!("{}[WARN]{} {}", ansi::YELLOW, ansi::RESET, message.as_ref());
}

/// Print an error callout banner
pub fn error(message: impl AsRef<str>) {
    println!("{}[ERROR]{} {}", ansi::RED, ansi::RESET, message.as_ref());
}

/// Print a note block
pub fn note(title: impl AsRef<str>, message: impl AsRef<str>) {
    println!("\n  {}{}{}: {}", ansi::BOLD, title.as_ref(), ansi::RESET, message.as_ref());
}
