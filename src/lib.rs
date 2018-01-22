extern crate ansi_term;

// Not a public API
#[doc(hidden)]
pub use std::sync::atomic::ATOMIC_USIZE_INIT;
use std::sync::atomic::AtomicUsize;

use std::fmt;
use std::iter::FromIterator;
use std::cmp;
use std::hash;

mod display;
mod colours;
mod codearg;

pub use codearg::CodeArg;

/// Mechanism for constructing a [`Code`] object. This macro takes a string
/// literal as its first argument, with `$substitutions`, and a series of
/// substitutions as the remaining arguments. Those substitutions can be
/// anything which implements the [`CodeArg`] trait.
///
/// [`Code`]: struct.Code.html
/// [`CodeArg`]: trait.CodeArg.html
///
/// # Example Usage
///
/// ```
/// # #[macro_use] extern crate cghelper;
/// # fn main() {
/// let body = code!(r#"
///     printf("This is the end of the world as we know it,\n");
///     printf("And my god it is starting to show it!\n");
/// "#);
///
/// let res = code!("
///     if ($cond) {
///         $body
///     }",
///     cond: "x == 5",
///     body: body,
/// );
///
/// assert_eq!(
///     res.to_string(),
///     "\
/// if (x == 5) {
///     printf(\"This is the end of the world as we know it,\\n\");
///     printf(\"And my god it is starting to show it!\\n\");
/// }"
/// );
/// # }
/// ```
#[macro_export]
macro_rules! code {
    ($e:expr) => { code!($e,) };
    ($e:expr, $($i:ident : $v:expr),* $(,)*) => {
        {
            static LOC: $crate::SourceLoc = $crate::SourceLoc {
                line: line!(),
                column: column!(),
                file: file!(),
                colour: $crate::ATOMIC_USIZE_INIT,
            };

            $crate::Code::build(
                $e, &LOC,
                &mut [ $(
                    $crate::BuildArg::new(stringify!($i), $v)
                ),* ]
            )
        }
    };
}

/// Internal datastructure used to represent how to construct a particular chunk
/// of Code.
#[cfg_attr(cghelper_internal_debug, derive(Debug))]
#[derive(Clone)]
enum Op {
    /// A newline character
    Nl,
    /// A string literal containing no newlines.
    Lit(&'static str),
    /// A dynamic blob, containing no newlines - `Box<str>` to keep `Op` small.
    Blob(Box<str>),

    /// An embedded `Code` object - `Box<[Op]>` to keep `Op` small.
    Inner(Box<[Op]>),
    /// A reference to another `Code` object which is being repeated.
    ///
    /// Encoded as an offset backward from the index of the current element.
    InnerRef(usize),

    /// Information about what source location the next chunk of code comes
    /// from.
    SourceLoc(&'static SourceLoc),
}

/// This struct represents a chunk of code.
///
/// Use the `Display` implementation on this type to transform your code into a
/// string output.
///
/// The "alternate" `Debug` implementation (enabled by using `"{:#?}"` in the
/// format string) on this type will be colorized to help with visualizing the
/// source of each piece of code.
///
/// # Example
///
/// ```
/// # #[macro_use] extern crate cghelper;
/// # fn main() {
/// let hello = code!("Hello");
/// let world = code!("World");
/// let result = code!("$hello, $world!", hello: hello, world: world);
/// println!("{:#?}", result);
/// # }
/// ```
#[cfg_attr(cghelper_internal_debug, derive(Debug))]
#[derive(Clone)]
pub struct Code {
    ops: Vec<Op>
}

impl Code {
    /// Create a new `Code` object containing no code.
    pub fn new() -> Self {
        Code { ops: vec![] }
    }

    /// Append the given [`CodeArg`].
    ///
    /// [`CodeArg`]: struct.CodeArg.html
    pub fn push<T: CodeArg>(&mut self, v: T) {
        self.ops.extend(v.into_code().ops)
    }

    // Not a public API - use code! instead.
    #[doc(hidden)]
    pub fn build(
        tmpl: &'static str,
        sourceloc: &'static SourceLoc,
        args: &mut [BuildArg],
    ) -> Self {
        str_to_code(tmpl, Some(sourceloc), Some(args), Op::Lit)
    }
}

impl<T> FromIterator<T> for Code
where
    T: CodeArg
{
    fn from_iter<I>(i: I) -> Code
    where
        I: IntoIterator<Item=T>,
    {
        let mut i = i.into_iter();
        let mut c = i.next()
            .map(|x| x.into_code())
            .unwrap_or(Code::new());
        for x in i { c.push(x); }
        c
    }
}

#[cfg(not(cghelper_internal_debug))]
impl fmt::Debug for Code {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if f.alternate() {
            f.write_str("Code {\n")?;
            display::do_display(self, f, 4, true)?;
            f.write_str("\n}")
        } else {
            f.write_str("Code {")?;
            display::do_display(self, f, 0, false)?;
            f.write_str("}")
        }
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        display::do_display(self, f, 0, false)
    }
}

// Not a public API
#[doc(hidden)]
#[derive(Debug)]
pub struct SourceLoc {
    pub line: u32,
    pub column: u32,
    pub file: &'static str,
    pub colour: AtomicUsize,
}

// NOTE: We want to compare SourceLoc objects by pointer, so we need custom hash
// and eq definitions.
impl cmp::PartialEq for SourceLoc {
    fn eq(&self, other: &Self) -> bool {
        self as *const Self == other as *const Self
    }
}
impl cmp::Eq for SourceLoc {}
impl hash::Hash for SourceLoc {
    fn hash<H: hash::Hasher>(&self, h: &mut H) {
        (self as *const Self).hash(h)
    }
}

/// Simple helper function to count the number of instances of a particular
/// character in a string. This is used to pre-allocate sufficiently large
/// buffers.
fn count_char(s: &str, c: char) -> usize {
    let mut count = 0;
    let mut idx = 0;
    while let Some(x) = s[idx..].find(c) {
        idx += x + c.len_utf8();
        count += 1;
    }
    count
}

/// Calculate the lowest indent (in charcters) of any line in the input string.
fn min_indent(s: &str) -> usize {
    let mut min_indent = usize::max_value();
    for line in s.lines() {
        // If we have a blank line, ignore it.
        let trimmed = line.trim_left();
        if trimmed.is_empty() { continue; }

        // Otherwise, indentation is the minimum of the length difference, and
        // min_indent.
        min_indent = usize::min(line.len() - trimmed.len(), min_indent);
    }
    min_indent
}

/// Find the next substitution point
fn subst_point(s: &str) -> Option<(&str, &str, &str)> {
    match s.find('$') {
        Some(x) => {
            let start = &s[..x];
            let s = &s[x+1..];
            let x = s.find(|x| match x {
                'a'...'z' | 'A'...'Z' | '0'...'9' | '_' => false,
                _ => true,
            }).unwrap_or(s.len());

            Some((start, &s[..x], &s[x..]))
        }
        None => None,
    }
}

// Not a public API
#[doc(hidden)]
pub struct BuildArg {
    name: &'static str,
    code: Option<Code>,
    index: usize,
}

impl BuildArg {
    // Not a public API
    #[doc(hidden)]
    pub fn new<T: CodeArg>(name: &'static str, arg: T) -> Self {
        BuildArg {
            name: name,
            code: Some(arg.into_code()),
            index: 0,
        }
    }
}

fn get_by_name<'a>(name: &str, args: &'a mut [BuildArg]) -> &'a mut BuildArg {
    for arg in args {
        if arg.name == name {
            return arg;
        }
    }
    panic!("No argument provided for substitution {}", name)
}

fn str_to_code<'a, F>(
    tmpl: &'a str,
    sourceloc: Option<&'static SourceLoc>,
    mut args: Option<&mut [BuildArg]>,
    mut str_op: F,
) -> Code
where
    F: FnMut(&'a str) -> Op
{
    // Come up with a size estimate. This should mean that we never need to
    // re-allocate our backing buffer.
    let mut estimate = count_char(tmpl, '\n') * 2 + 1;
    if args.is_some() {
        estimate += count_char(tmpl, '$') * 2;
    }
    if sourceloc.is_some() {
        estimate += 1;
    }

    let mut ops = Vec::with_capacity(estimate);
    if let Some(sourceloc) = sourceloc {
        ops.push(Op::SourceLoc(sourceloc));
    }

    let indent = min_indent(tmpl);

    // NOTE: We use .split('\n') rather than .lines here because we want to
    // handle the last newline correctly.
    for (idx, mut line) in tmpl.split('\n').enumerate() {
        if idx != 0 {
            ops.push(Op::Nl);
        }

        // Remove any common indent prefix, and remove trailing whitespace.
        if line.len() >= indent {
            line = &line[indent..];
        }
        line = line.trim_right();
        if line.is_empty() {
            continue;
        }

        if let Some(ref mut args) = args {
            while let Some((b, name, r)) = subst_point(line) {
                line = r;
                if !b.is_empty() {
                    ops.push(str_op(b));
                }

                let arg = get_by_name(name, args);
                if let Some(code) = arg.code.take() {
                    arg.index = ops.len();
                    ops.push(Op::Inner(code.ops.into_boxed_slice()));
                } else {
                    let off = ops.len() - arg.index;
                    ops.push(Op::InnerRef(off));
                }
            }
        }

        if !line.is_empty() {
            ops.push(str_op(line));
        }
    }

    debug_assert!(estimate >= ops.len());
    Code { ops }
}
