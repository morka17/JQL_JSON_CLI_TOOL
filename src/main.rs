extern crate nom;

use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{char, digit1},
    combinator::{map_res, opt, recognize, value},
    error::{FromExternalError, ParseError},
    sequence::tuple,
    IResult, Parser,
};

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

fn main() {
    println!("Hello, world!");
}
