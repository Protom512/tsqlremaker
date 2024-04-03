use nom::branch::alt;
use nom::bytes::complete::{tag_no_case, take_while, take_while1};
use nom::character::complete::{digit0, digit1, multispace0, multispace1};
use nom::character::{is_alphanumeric, is_digit};
use nom::combinator::{map, opt};
use nom::{
    bytes::complete::{tag, take_while_m_n},
    combinator::map_res,
    sequence::tuple,
    IResult,
};
use regex::Regex;

pub enum Operator {
    SELECT,
    UPDATE,
    DELETE,
    INSERT,
    CREATE,
}
/// ```BNF
/// table_factor ::= NUMBER_LITERAL | '(' expr ')'
/// term ::= factor | factor ('*'|'/') factor
/// expr ::= term   | term   ('+'|'-') term
/// ```
///
#[derive(Debug, PartialEq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

pub struct Procedure<'a> {
    /// プロシージャ名
    proc_name: &'a str,
    SQL_statement: SQL,
}
#[derive(Debug, PartialEq)]
pub struct SQL {
    queries: Vec<query>,
}
#[derive(Debug, PartialEq)]
pub struct query {
    queries: Vec<query>,
}

/// ```bnf
/// create [or replace] procedure [owner.]procedure_name[;number]
/// 	[[(@parameter_name datatype [(length) | (precision [, scale])]
/// 		[= default][output]
/// 	[, @parameter_name datatype [(length) | (precision [, scale])]
/// 		[= default][output]]...)]]
/// 	[with {recompile | execute as {owner | caller}} ]
/// 	as {SQL_statements | external name dll_name}
///```
///
pub struct SP {}

///```bnf
/// insert [into] [database.[owner.]]{table_name|view_name}
/// 	[(column_list)]
/// 	{values (expression [, expression]...)
/// 		|select_statement [plan "abstract plan"]}
/// ```

///
/// ```bnf
/// select ::=
/// 	select [all | distinct]
/// 	[top unsigned_integer]
/// 	select_list
/// 	[into_clause]
/// 	[from_clause]
/// 	[where_clause]
/// 	[group_by_clause]
/// 	[having_clause]
/// 	[order_by_clause]
/// 	[compute_clause]
/// 	[read_only_clause]
/// 	[isolation_clause]
/// 	[browse_clause]
/// 	[plan_clause]
/// 	[for_xml_clause]
///
/// select_list ::=
///```
///
///

#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub enum select_all_distict {
    All,
    Distinct,
    None,
}
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct Select<'a> {
    all_or_distinct: select_all_distict,
    top_int: Option<u8>,
    select_list: &'a str,
    into_clause: Option<&'a str>,
    from_clause: Option<&'a str>,
    where_clause: Option<&'a str>,
    group_by_clause: Option<&'a str>,
    having_clause: Option<&'a str>,
    order_by_clause: Option<&'a str>,
    compute_clause: Option<&'a str>,
}
// fn from_hex(input: &str) -> Result<u8, std::num::ParseIntError> {
//     u8::from_str_radix(input, 16)
// }
//
// fn is_hex_digit(c: char) -> bool {
//     c.is_digit(16)
// }
//
// fn hex_primary(input: &str) -> IResult<&str, u8> {
//     map_res(take_while_m_n(2, 2, is_hex_digit), from_hex)(input)
// }
//
// fn hex_color(input: &str) -> IResult<&str, Color> {
//     let (input, _) = tag("#")(input)?;
//     let (input, (red, green, blue)) = tuple((hex_primary, hex_primary, hex_primary))(input)?;
//
//     Ok((input, Color { red, green, blue }))
// }

fn is_ident(ch: char) -> bool {
    is_alphanumeric(ch as u8) || ch == '_' || ch == '-'
}

pub fn is_char_digit(chr: char) -> bool {
    return chr.is_ascii() && is_digit(chr as u8);
}
fn parse_select(input: &str) -> IResult<&str, &str> {
    map(
        tuple((
            multispace0,
            tag_no_case("select"),
            multispace1,
            opt(tuple((
                alt((tag_no_case("all"), tag_no_case("distinct"))),
                multispace1,
            ))),
            opt(tuple((
                tag_no_case("top"),
                multispace1,
                take_while1(is_char_digit),
                multispace1,
            ))),
        )),
        |(_, select_phrase, _, select_all_distinct_phrase, top_phrase)| {
            // let alldist:select_all_distict =match select_all_distinct_phrase {
            //     Some((n,_)) => {
            //         match(n.to_uppercase().as_str()){
            //             "ALL" => select_all_distict::All,
            //             "DISTINCT" => select_all_distict::Distinct,
            //             _ => panic!("{:?}",n)
            //         }
            //         dbg!(n)

            //     }
            //     None => select_all_distict::None
            // };
            // let top = match(top_phrase){
            //     Some((_,m,_,j))=>{Some(j.parse().unwrap())},
            //     None=>None,

            // };
            // let result=Select{
            //    all_or_distinct: alldist,
            //     top_int: top,

            // };

            // ( select_phrase, alldist, top_phrase)
            select_phrase
        },
    )(input)
}
fn remove_comments(input: &str) -> String {
    let re = Regex::new(r"--.*\n?").unwrap();
    re.replace_all(input, "\n").into_owned()
}

fn sp(input: &str) -> IResult<&str, &str> {
    // let input_Str = remove_comments(input);
    let (input, OB) = parse_select(input)?;

    Ok((input, OB))
}

fn main() {
    unimplemented!()
}
#[cfg(test)]
mod tests {
    use nom::IResult;
    use rstest::rstest;

    use crate::sp;

    #[rstest]
    #[test]
    #[case("select * from test01db",Ok(("* from test01db", "select")))]
    #[case("select distinct * from test01db",Ok(("* from test01db", "select")))]
    #[case("select top 10  * from test01db",Ok(("* from test01db", "select")))]
    #[test]
    fn test_rstest_selct(#[case] sql: &str, #[case] result: IResult<&str, &str>) {
        assert_eq!(sp(sql), result);
    }
}

// #[test]
// fn test_simple_comment() {
//     let sql = "select * -- hoge comment \
//     from test01db";
//     assert_eq!(sp(sql), Ok(("* from test01db", "select")));
// }
