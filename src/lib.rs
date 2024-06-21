use nom::branch::alt;
use nom::bytes::complete::{tag_no_case, take_while1};
use nom::character::complete::{multispace0, multispace1};
use nom::character::{is_alphanumeric, is_digit};
use nom::combinator::{map, opt};
use nom::{sequence::tuple, IResult};
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

pub struct Procedure<'a> {
    /// プロシージャ名
    proc_name: &'a str,
    sql_statement: SQL,
}
#[derive(Debug, PartialEq)]
pub struct SQL {
    queries: Vec<Query>,
}
#[derive(Debug, PartialEq)]
pub struct Query {
    queries: Vec<Query>,
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
pub enum SelectAllDistict {
    All,
    Distinct,
    None,
}
#[derive(Debug, Ord, PartialOrd, Eq, PartialEq)]
pub struct Select<'a> {
    all_or_distinct: Option<&'a str>,
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

/// .
///
/// # Errors
///
/// This function will return an error if .
fn parse_select_into(input: &str) -> IResult<&str, &str> {
    let (input, opt_into) = opt(tuple((multispace0, tag_no_case("into"), multispace1)))(input)?;
    let into = match opt_into {
        Some((_, m, _)) => m,
        None => "",
    };
    Ok((input, into))
}
fn is_ident(ch: char) -> bool {
    is_alphanumeric(ch as u8) || ch == '_' || ch == '-'
}

fn parse_select_top(input: &str) -> IResult<&str, Option<u8>> {
    let (input, opt_top) = opt(tuple((
        tag_no_case("top"),
        multispace1,
        take_while1(is_char_digit),
        multispace1,
    )))(input)?;
    let num: Option<u8> = opt_top.map(|(m, _, num, _)| num.parse().expect("failed to convert"));
    Ok((input, num))
}

fn parse_select_ditinct(input: &str) -> IResult<&str, &str> {
    let (input, opt_top) = opt(tuple((
        multispace0,
        alt((tag_no_case("all"), tag_no_case("distinct"))),
        multispace1,
    )))(input)?;
    dbg!("{:#?}", opt_top);
    let top = match opt_top {
        Some((n, m, p)) => m,
        None => "",
    };
    Ok((input, top))
}
pub fn is_char_digit(chr: char) -> bool {
    chr.is_ascii() && is_digit(chr as u8)
}
fn parse_select(input: &str) -> IResult<&str, &str> {
    map(
        tuple((
            multispace0,
            tag_no_case("select"),
            multispace1,
            parse_select_ditinct,
            parse_select_top,
        )),
        |(_, select_phrase, _, select_all_distinct_phrase, top_phrase)| select_phrase,
    )(input)
}

/// remove_comments
///
/// # Arguments
///
/// * `arg1`: sql text 
///
/// # Returns
///
/// String: コメント削除されたSqlテキスト
///
/// # Examples
///
/// ```
/// // 使用例をここに示します。
/// let sql_text="select hoge -- comment
/// from table";
/// let input_txt = remove_comments(sql_text);
/// assert_eq!(input_txt, "select hoge from table");
/// ```
///
/// # Panics
///
/// 関数がパニックする条件や状況について説明します。パニックしない場合はこのセクションを省略します。
/// TBD
/// 
/// # Errors
///
/// エラーが発生する可能性がある場合、その詳細をここに記述します。エラーが発生しない場合はこのセクションを省略します。
/// TBD
/// 
/// # Safety
///
/// `unsafe`な関数の場合、なぜ安全性が保証されているのかを説明します。安全性に関する特記事項がない場合はこのセクションを省略します。
/// TBD
/// # Notes
///
/// 追加の注記や関連する情報があればここに記述します。
///
/// # See also
///
/// 他に関連する関数やドキュメントへのリンクを記述します。
///
fn remove_comments(input: &str) -> String {
    /// 一行のみのコメントを削除する
    ///
    let re = Regex::new(
        // r"(?: (?:'[^']*?') | (?<singleline>\s*--\s*[^\n]*) | (?<multiline>(?:\/\*)+?[\w\W]+?(?:\*\/)+) )",
        r"\s*--\s*.*[\r\n]+",
    )
    .expect("single line comments removal regex failed");
    let input2 = re.replace_all(input, "").into_owned();
    let re = Regex::new(r#"[\r\n]+"#).expect("regex init on newline  failed");
    let temp_input = re.replace_all(input2.as_str(), "").into_owned();
    let re = Regex::new(r#"\/\*.*\*\/"#).expect("multi line comments removal regex failed");
    re.replace_all(temp_input.as_str(), "").into_owned()
}

fn sp(input: &str) -> IResult<&str, &str> {
    let input_Str = remove_comments(input);
    let (input, ob) = parse_select(input)?;

    Ok((input, ob))
}

#[cfg(test)]
mod tests {
    use nom::IResult;
    use rstest::rstest;

    use crate::sp;
    use crate::{parse_select_ditinct, parse_select_into, parse_select_top, remove_comments};

    #[rstest]
    #[test]
    #[case("select * from test01db",Ok(("* from test01db", "select")))]
    #[case("select distinct * from test01db",Ok(("* from test01db", "select")))]
    #[case("select top 10  * from test01db",Ok(("* from test01db", "select")))]
    fn test_rstest_selct(#[case] sql: &str, #[case] result: IResult<&str, &str>) {
        assert_eq!(sp(sql), result);
    }
    #[rstest]
    #[test]
    #[case("all * ",Ok(("* ", "all")))]
    #[case("  all * ",Ok(("* ", "all")))]
    #[case("All * ",Ok(("* ", "All")))]
    #[case("  All * ",Ok(("* ", "All")))]
    #[case("distinct * ",Ok(("* ", "distinct")))]
    #[case("  distinct * ",Ok(("* ", "distinct")))]
    #[case("   * ",Ok(("   * ", "")))]
    fn test_rstest_parse_select_ditinct(#[case] sql: &str, #[case] result: IResult<&str, &str>) {
        assert_eq!(parse_select_ditinct(sql), result);
    }

    #[rstest]
    #[test]
    #[case("top 1 * ",Ok(("* ", Some(1))))]

    fn test_rstest_parse_select_top(#[case] sql: &str, #[case] result: IResult<&str, Option<u8>>) {
        assert_eq!(parse_select_top(sql), result);
    }
    #[rstest]
    #[test]
    #[case("into * ",Ok(("* ", "into")))]

    fn test_rstest_parse_select_into(#[case] sql: &str, #[case] result: IResult<&str, &str>) {
        assert_eq!(parse_select_into(sql), result);
    }
    #[rstest]
    #[test]
    #[case(
        r"
    SELECT 1 --hoge
    ",
        "    SELECT 1    "
    )]
    #[case(
        r"/*comment1*/*comment2*/comment1end*/
    SELECT 1 
    ",
        "    SELECT 1     "
    )]
    #[case(
        r"/*comment1
    */
    SELECT 1 
    ",
        "    SELECT 1     "
    )]
    fn test_rstest_comments_(#[case] sql: &str, #[case] expect: String) {
        let res = remove_comments(sql);
        assert_eq!(res, expect);
    }
}
