use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "morph.pest"]
struct MorphParser;

fn main() {
    let input = r#"// Comment
enum enum1: i16 {
    apple = 1,
    orange = 2,
    kiwiFruit = 3,
    pear, // = 3 is inferred
}
// Another comment
struct type1 {
    alpha: i8,
    alpha_beta: u8,
    alphaBeta: i16,
    a4: u16,
    a5: i32,
    a6: u32,
    a7: i64,
    a8: u64,
    a9: f32,
    a10: f64,
    n1: i8?,
    n2: u8?,
    n3: i16?,
    n4: u16?,
    n5: i16?,
    n6: u16?,
    n7: i32?,
    n8: u32?,
    n9: i64?,
    n10: u64?,
    s1: string,
    s2: string?,
    b1: bool,
    b2: bool?,
    e1: enum1,
    e2: enum1?,
    r1: [ string ],
    r2: [ string ]?,
    r2: [ string; 10],
    m1: { string : f64 },
    s1: type1,
};"#;

    match MorphParser::parse(Rule::file, input) {
        Ok(pairs) => {
            for pair in pairs {
                println!("{:#?}", pair);
            }
        }
        Err(e) => eprintln!("Parse error:\n{}", e),
    }
}
