//! This module provides the implementation of Display for Code.

use super::*;

/// A limiter on the maximum number of consecutive newlines. This reduces the
/// number of unnecessary newlines which are generated in the target file,
/// making the output nicer to read.
const MAX_CONSECUTIVE_NEWLINES: usize = 2;

#[derive(Default)]
struct State {
    curr: String,
    nls: usize,
    // NOTE: Default value is good for max_nls, as we don't want to generate any
    // leading newlines in the final output.
    max_nls: usize,
    offset: usize,
}

impl State {
    fn run(
        &mut self,
        f: &mut fmt::Formatter,
        ops: &[Op],
        base_offset: usize
    ) -> fmt::Result {
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

                Op::SourceLoc(..) => {}
            }
        }
        Ok(())
    }

    fn flush(&mut self, f: &mut fmt::Formatter, base_offset: usize) -> fmt::Result {
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
            f.write_str(&self.curr)?;

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

        Ok(())
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut state = State::default();
        state.run(f, &self.ops, 0)?;
        state.flush(f, 0)
    }
}
