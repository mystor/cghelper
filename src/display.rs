//! This module provides the implementation of Display for Code.

use super::*;

use ansi_term::Style;
use std::collections::HashSet;

/// A limiter on the maximum number of consecutive newlines. This reduces the
/// number of unnecessary newlines which are generated in the target file,
/// making the output nicer to read.
const MAX_CONSECUTIVE_NEWLINES: usize = 2;

struct State {
    curr: String,
    nls: usize,
    // NOTE: Default value is good for max_nls, as we don't want to generate any
    // leading newlines in the final output.
    max_nls: usize,
    offset: usize,
    styles: Option<(Vec<(usize, Style)>, HashSet<&'static SourceLoc>)>,
}

impl State {
    fn new(debug_highlight: bool) -> Self {
        State {
            curr: String::new(),
            nls: 0,

            // Don't generate any leading newlines in the final output
            max_nls: 0,
            offset: 0,

            styles: if debug_highlight {
                // Start with the default style.
                Some((vec![(0, Style::default())], HashSet::new()))
            } else {
                None
            },
        }
    }

    fn run(
        &mut self,
        f: &mut fmt::Formatter,
        ops: &[Op],
        base_offset: usize,
    ) -> fmt::Result {
        let restore_style = if let Some((ref mut styles, _)) = self.styles {
            let (_, restore_style) = *styles.last().unwrap();

            // If no styles are applied, it's a basic substitution. Make the text
            // bold and underlined.
            styles.push((self.curr.len(), restore_style.bold().underline()));
            Some(restore_style)
        } else {
            None
        };

        for (idx, op) in ops.iter().enumerate() {
            match *op {
                Op::Nl => {
                    self.flush(f, base_offset)?;

                    // Record that we have seen an additional newline, and clamp
                    // the maximum number of consecutive newlines to
                    // MAX_CONSECUTIVE_NEWLINES.
                    if self.nls < self.max_nls {
                        self.nls += 1;
                    }
                }

                Op::Lit(ref seg) => {
                    self.offset += seg.len();
                    self.curr.push_str(seg);
                }
                Op::Blob(ref seg) => {
                    self.offset += seg.len();
                    self.curr.push_str(seg);
                }

                Op::Inner(ref inner) => {
                    let offset = self.offset;
                    self.run(f, inner, offset)?;
                }

                Op::InnerRef(back) => {
                    let offset = self.offset;
                    assert!(back <= idx, "Invalid index");
                    match ops[idx - back] {
                        Op::Inner(ref inner) => {
                            self.run(f, inner, offset)?;
                        }
                        _ => panic!("Invalid type at index"),
                    }
                }

                Op::SourceLoc(sourceloc) => {
                    if let Some((ref mut styles, ref mut seen)) = self.styles {
                        styles.push((self.curr.len(), sourceloc.style()));
                        seen.insert(sourceloc);
                    }
                }
            }
        }

        if let Some((ref mut styles, _)) = self.styles {
           styles.push((self.curr.len(), restore_style.unwrap()));
        }
        Ok(())
    }

    fn flush(
        &mut self,
        f: &mut fmt::Formatter,
        base_offset: usize,
    ) -> fmt::Result {
        use std::fmt::Write;

        // If we have a non-blank line, flush it.
        if !self.curr.chars().all(char::is_whitespace) {
            // XXX(hacky?): Don't generate more than 1 newline before a line
            // starting with a closing brace.
            if self.curr.trim_left().starts_with(&['}', ')', ']'][..]) {
                self.nls = usize::min(self.nls, 1);
            }

            for _ in 0..self.nls { f.write_char('\n')?; }
            self.nls = 0;

            if let Some((ref styles, _)) = self.styles {
                // We're styling, make sure to write out the correct styles!
                let mut c = 0;
                let mut style = Style::default();
                for &(idx, new_style) in styles {
                    write!(f, "{}", style.paint(&self.curr[c..idx]))?;
                    c = idx;
                    style = new_style;
                }
                write!(f, "{}", style.paint(&self.curr[c..]))?;
            } else {
                // Not styling - we don't have to write out styles.
                f.write_str(&self.curr)?;
            }

            // XXX(hacky?): Don't generate more than 1 newline after a line
            // starting with a curly brace.
            if self.curr.trim_right().ends_with(&['{', '(', '['][..]) {
                self.max_nls = 1;
            } else {
                self.max_nls = MAX_CONSECUTIVE_NEWLINES;
            }
        }

        // Reset our offset.
        self.offset = base_offset;

        // Reset curr to the base offset
        self.curr.clear();
        self.curr.reserve(self.offset);
        for _ in 0..self.offset { self.curr.push(' '); }

        if let Some((ref mut styles, _)) = self.styles {
            // Reset the styles array.
            let len = styles.len();
            if len > 1 {
                styles.drain(1..len-1);
                styles[1].0 = self.curr.len();
            }
        }

        Ok(())
    }
}

pub(crate) fn do_display(
    code: &Code,
    f: &mut fmt::Formatter,
    indent: usize,
    debug_highlight: bool,
) -> fmt::Result {
    let mut state = State::new(debug_highlight);
    for _ in 0..indent { state.curr.push(' '); }
    state.run(f, &code.ops, indent)?;
    state.flush(f, 0)?;

    if let Some((_, ref seen)) = state.styles {
        write!(f, "{}", Style::new().bold().paint("\n  LEGEND"))?;
        for seen in seen {
            let entry = seen.style().paint(format!("{}:{}:{}", seen.file, seen.line, seen.column));
            write!(f, "\n    {}", entry)?;
        }
    }
    Ok(())
}
