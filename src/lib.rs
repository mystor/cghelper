use std::fmt;
use std::iter::FromIterator;

mod display;

// XXX(todo): Consider providing an option to discover where a piece of the
// output code is coming from in your code. Potentially a method which performs
// debug printing but with annotations about what line/column produced each
// part?

// Not a public API
#[doc(hidden)]
#[derive(Debug)]
pub struct SourceLoc {
    pub line: u32,
    pub column: u32,
    pub file: &'static str,
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
    /// A dynamic blob, containing no newlines.
    Blob(Box<str>),

    /// An embedded `Code` object.
    Inner(Box<[Op]>),
    /// A reference to another `Code` object which is being repeated.
    ///
    /// Encoded as an offset backward from the index of the current element.
    InnerRef(usize),

    /// Information about what source location the next chunk of code comes
    /// from.
    SourceLoc(&'static SourceLoc),
}

///
#[cfg_attr(cghelper_internal_debug, derive(Debug))]
#[derive(Clone)]
pub struct Code {
    ops: Vec<Op>
}

#[cfg(not(cghelper_internal_debug))]
impl fmt::Debug for Code {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Code {{{}}}", self)
    }
}

#[doc(hidden)]
pub trait CodeArg {
    fn into_code(self) -> Code;
}

impl CodeArg for Code {
    fn into_code(self) -> Code {
        self
    }
}

impl CodeArg for bool {
    fn into_code(self) -> Code {
        Code {
             ops: vec![ Op::Lit(if self { "true" } else { "false" }) ]
        }
    }
}

impl CodeArg for String {
    fn into_code(self) -> Code {
        // We won't be performing any transformations on this buffer, so let's
        // just save the string directly.
        if !self.contains('\n') && self.chars().map(char::is_whitespace).next().unwrap_or(false) {
            Code {
                ops: vec![ Op::Blob(self.into_boxed_str()) ],
            }
        } else {
            (&self[..]).into_code()
        }
    }
}

impl<'a> CodeArg for &'a str {
    fn into_code(self) -> Code {
        str_to_code(
            self,
            None,
            None,
            |s| Op::Blob(s.to_owned().into_boxed_str()),
        )
    }
}

macro_rules! codearg_display {($($i:ident),*) => {
    $( impl CodeArg for $i {
        fn into_code(self) -> Code {
            // We know that the strings won't contain '\n' or any leading
            // whitespace, so we can skip that test.
            Code {
                ops: vec![ Op::Blob(self.to_string().into_boxed_str()) ],
            }
        }
    } )*
}}
codearg_display! { i8, i16, i32, i64, u8, u16, u32, u64, f32, f64 }

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
        estimate += count_char(tmpl, '$');
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

impl Code {
    pub fn new() -> Self {
        Code { ops: vec![] }
    }

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

/// Mechanism for constructing a [`Code`] object. This macro takes a string
/// literal as its first argument, with `$substitutions`, and a series of
/// substitutions as the remaining arguments. Those substitutions can be
/// anything which implements the [`CodeArg`] trait.
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
