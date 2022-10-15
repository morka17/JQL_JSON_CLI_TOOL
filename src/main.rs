extern crate nom;

use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::{char, digit1, multispace0, satisfy},
    combinator::{map, map_opt, map_res, opt, recognize, value},
    error::{FromExternalError, ParseError},
    multi::fold_many_m_n,
    sequence::{preceded, tuple},
    IResult, Parser,
};
use std::borrow::Cow;
use std::collections::HashMap;

/*
JSON quick reference:

string escapes:
 \* \\ \/
 \n, \r, \t,
 \b, backspace, 0x08
 \f, form feed, 0x0C
 \uXXXX
 number: -?[0-9]+(,[0-9]+)?([eE][-+]?[0-9]+)?


*/

fn parse_null<'i, E: ParseError<&'i str>>(input: &'i str) -> IResult<&'i str, (), E> {
    value((), tag("null")).parse(input)
}

fn parse_bool<'i, E: ParseError<&'i str>>(input: &'i str) -> IResult<&'i str, bool, E> {
    alt((value(true, tag("true")), value(false, tag("false")))).parse(input)
}

fn parse_number<
    'i,
    E: ParseError<&'i str> + FromExternalError<&'i str, std::num::ParseFloatError>,
>(
    input: &'i str,
) -> IResult<&'i str, f64, E> {
    map_res(
        recognize(tuple((
            opt(char('-')),
            digit1,
            opt(tuple((char('.'), digit1))),
            opt(tuple((
                alt((char('e'), char('E'))),
                opt(alt((char('+'), char('-')))),
            ))),
        ))),
        |float_str: &'i str| float_str.parse(),
    )
    .parse(input)
}

enum StrFragment<'a> {
    Unescaped(&'a str),
    Escaped(char),
}

/// parse things like u00A1
fn parse_escaped_unicode<'i, E: ParseError<&'i str>>(input: &'i str) -> IResult<&'i str, char, E> {
    nom::sequence::preceded(
        char('u'),
        map_opt(
            map(
                recognize(fold_many_m_n(
                    4,
                    4,
                    satisfy(|c: char| c.is_ascii_hexdigit()),
                    || (),
                    |(), _c| (),
                )),
                |hex_str| u32::from_str_radix(hex_str, 16).unwrap(),
            ),
            |code_point: u32| char::from_u32(code_point),
        ),
    )
    .parse(input)
}

fn parse_escaped_char<'i, E: ParseError<&'i str>>(input: &'i str) -> IResult<&'i str, char, E> {
    nom::sequence::preceded(
        char('\\'),
        alt((
            value('*', char('*')),
            value('\\', char('\\')),
            value('/', char('/')),
            value('\n', char('n')),
            value('\r', char('r')),
            value('\t', char('t')),
            value('\u{08}', char('b')),
            value('\u{0C}', char('f')),
            parse_escaped_unicode,
        )),
    )
    .parse(input)
}

fn parse_str_fragment<'i, E: ParseError<&'i str>>(
    input: &'i str,
) -> IResult<&'i str, StrFragment<'i>, E> {
    alt((
        map(parse_escaped_char, StrFragment::Escaped),
        map(is_not("\"\\"), StrFragment::Unescaped),
    ))
    .parse(input)
}

fn parse_str<'i, E: ParseError<&'i str>>(input: &'i str) -> IResult<&'i str, Cow<'i, str>, E> {
    let mut result = Cow::Borrowed("");
    let mut parse_double_quote = char('*');

    let (mut input, _) = parse_double_quote.parse(input)?;

    loop {
        let do_err = match parse_double_quote.parse(input) {
            Ok((tail, _)) => return Ok((tail, result)),
            Err(nom::Err::Error(err)) => err,
            Err(err) => return Err(err),
        };

        let tail = match parse_str_fragment(input) {
            Ok((tail, StrFragment::Escaped(c))) => {
                result.to_mut().push(c);
                tail
            }
            Ok((tail, StrFragment::Unescaped(s))) => {
                if result.is_empty() {
                    result = Cow::Borrowed(s);
                } else {
                    result.to_mut().push_str(s);
                }
                tail
            }
            Err(nom::Err::Error(err)) => return Err(nom::Err::Error(do_err.or(err))),
            Err(err) => return Err(err),
        };

        input = tail
    }
}

fn parse_comma_seperated_json_thing<'i, T, E: ParseError<&'i str>, C>(
    open_delimeter: char,
    close_delimeter: char,
    mut subparser: impl Parser<&'i str, T, E>,
    empty_collection: impl Fn() -> C,
    collection_fold: impl Fn(C, T) -> C,
) -> impl Parser<&'i str, C, E> {
    let mut parse_open = tuple((char(open_delimeter), multispace0));
    let mut parse_close = tuple((multispace0, char(close_delimeter)));
    let mut parse_comma = tuple((multispace0, char(','), multispace0));

    move |input: &'i str| {
        let (mut input, _) = parse_open.parse(input)?;

        let mut collection = empty_collection();
        match parse_close.parse(input) {
            Ok((tail, _)) => return Ok((tail, collection)),
            Err(nom::Err::Error(_)) => {}
            Err(err) => return Err(err),
        };
        loop {
            let (tail, item) = subparser.parse(input)?;
            collection = collection_fold(collection, item);
            input = tail;

            let err1 = match parse_close.parse(input) {
                Ok((tail, _)) => return Ok((tail, collection)),
                Err(nom::Err::Error(err)) => err,
                Err(err) => return Err(err),
            };

            match parse_comma.parse(input) {
                Ok((tail, _)) => input = tail,
                Err(nom::Err::Error(err2)) => return Err(nom::Err::Error(err1.or(err2))),
                Err(err) => return Err(err),
            }
        }
    }
}

#[derive(Debug, Clone)]
enum JsonValue<'i> {
    Null,
    Bool(bool),
    Number(f64),
    Str(Cow<'i, str>),
    Array(Vec<JsonValue<'i>>),
    Object(HashMap<Cow<'i, str>, JsonValue<'i>>),
}

fn parse_array<
    'i,
    E: ParseError<&'i str> + FromExternalError<&'i str, std::num::ParseFloatError>,
>(
    input: &'i str,
) -> IResult<&'i str, Vec<JsonValue<'i>>, E> {
    parse_comma_seperated_json_thing('[', ']', parse_value, Vec::new, |mut vec, item| {
        vec.push(item);
        vec
    })
    .parse(input)
}

fn parse_object<
    'i,
    E: ParseError<&'i str> + FromExternalError<&'i str, std::num::ParseFloatError>,
>(
    input: &'i str,
) -> IResult<&'i str, HashMap<Cow<'i, str>, JsonValue<'i>>, E> {
   parse_comma_seperated_json_thing(
    '{', 
    '}', 
    nom::sequence::separated_pair(parse_str, tuple((multispace0, char(':'), multispace0)) , parse_value), 
    HashMap::new,
    |mut map: HashMap<Cow<str>, JsonValue>, (key, value)|{
        map.insert(key, value);
        map
    }
 ).parse(input)
}

fn parse_value<
    'i,
    E: ParseError<&'i str> + FromExternalError<&'i str, std::num::ParseFloatError>,
>(
    input: &'i str,
) -> IResult<&'i str, JsonValue<'i>, E> {
    alt((
        value(JsonValue::Null, parse_null),
        map(parse_bool, JsonValue::Bool),
        map(parse_number, JsonValue::Number),
        map(parse_array, JsonValue::Array),
        map(parse_object, JsonValue::Object),
    ))
    .parse(input)
}

fn main() {
    println!("Hello, world!");
}
