#[macro_use]
extern crate cghelper;

use cghelper::Code;

struct Struct<'a> {
    name: &'a str,
    fields: &'a [(&'a str, &'a str)],
}

fn gen_fields_code(fields: &[(&str, &str)]) -> Code {
    fields.iter().map(|&(name, ty)| {
        code! {
            "$ty $name;\n",
            ty: ty,
            name: name,
        }
    }).collect()
}

fn gen_struct_code(structs: &[Struct]) -> Code {
    structs.iter().map(|s| {
        let fields = gen_fields_code(s.fields);
        code!("
            struct $name {
                $fields
            };
            ",
            name: s.name,
            fields: fields,
        )
    }).collect()
}

fn main() {
    let inclguard = "_some_file_h";
    let structs = &[
        Struct {
            name: "peaches",
            fields: &[
                ("a", "uint32_t"),
                ("b", "char*"),
            ],
        },
        Struct {
            name: "celery",
            fields: &[
                ("a", "uint32_t"),
                ("b", "char*"),
            ],
        }
    ];

    let struct_code = gen_struct_code(structs);

    let code = code!("
        // This file is generated

        #ifndef $inclguard
        #define $inclguard

        $structs

        #endif // defined($inclguard)
        ",
        inclguard: inclguard,
        structs: struct_code,
    );

    println!("code = {:#?}", code);

    println!("{}", code);
}