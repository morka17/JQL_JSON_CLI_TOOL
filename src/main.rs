extern crate nom;

use nom::{
    branch::alt, bytes::complete::tag, combinator::value, error::ParseError, IResult, Parser,
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

fn main() {
    println!("Hello, world!");
}
