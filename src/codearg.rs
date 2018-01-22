use {Code, Op, str_to_code};

/// Objects which implement this trait can be converted into [`Code`] objects.
/// This allows them to be used as arguments to the [`code!`] macro.
///
/// [`Code`]: struct.Code.html
/// [`code!`]: macro.code.html
pub trait CodeArg {
    /// Convert this object into a `Code` object.
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
        if !self.contains('\n') && self.chars().map(char::is_whitespace).next().unwrap_or(false) {
            // We won't be performing any transformations on this buffer, so
            // let's just save the string directly, saving allocations.
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
