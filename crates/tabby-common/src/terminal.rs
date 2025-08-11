pub enum HeaderFormat {
    BoldWhite,
    BoldBlue,
    BoldYellow,
    BoldRed,
    Blue,
}

impl HeaderFormat {
    fn prefix(&self) -> &str {
        match self {
            HeaderFormat::BoldWhite => "\x1b[1m",
            HeaderFormat::BoldBlue => "\x1b[34;1m",
            HeaderFormat::BoldYellow => "\x1b[93;1m",
            HeaderFormat::Blue => "\x1b[34m",
            HeaderFormat::BoldRed => "\x1b[1;31m",
        }
    }

    pub fn format(&self, header: &str) -> String {
        format!("{}{header}\x1b[0m", self.prefix())
    }
}

/// Message for displaying formatted information to users
/// 
/// The 'display lifetime represents the lifetime of the text data
/// being formatted and displayed.
pub struct InfoMessage<'display> {
    header: &'display str,
    header_format: HeaderFormat,
    lines: &'display [&'display str],
}

impl<'display> InfoMessage<'display> {
    pub fn new(header: &'display str, header_format: HeaderFormat, lines: &'display [&'display str]) -> Self {
        Self {
            header,
            header_format,
            lines,
        }
    }

    pub fn print(self) {
        eprintln!("\n{}\n", self.to_string());
    }

    pub fn print_messages(messages: &[Self]) {
        let messages: Vec<String> = messages.iter().map(|m| m.to_string()).collect();
        eprintln!("\n{}\n", messages.join("\n"));
    }
}

impl<'display> ToString for InfoMessage<'display> {
    fn to_string(&self) -> String {
        let mut str = String::new();
        str.push_str(&format!("  {}\n\n", self.header_format.format(self.header)));
        for (i, line) in self.lines.iter().enumerate() {
            str.push_str("  ");
            str.push_str(line);
            // TODO: REQUIRES CLARIFICATION!
            // What result is expected:
            // A) "line1\nline2\nline3" (without hyphenation at the end)
            // B) "line1\nline2\nline3\n" (with hyphenation at the end)
            //
            // The current code contains an error: condition i != len + 1 is always true
            // because self.lines.len() is always > 0
            if i != self.lines.len() - 1 { // Temporary fix for case A
                str.push('\n');
            }
        }
        str
    }
}
