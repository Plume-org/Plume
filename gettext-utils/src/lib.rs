#[derive(Debug)]
#[doc(hidden)]
pub enum FormatError {
    UnmatchedCurlyBracket,
    InvalidPositionalArgument,
}

#[doc(hidden)]
pub fn try_format<'a>(
    str_pattern: &'a str,
    argv: &[::std::boxed::Box<dyn ::std::fmt::Display + 'a>],
) -> ::std::result::Result<::std::string::String, FormatError> {
    use ::std::fmt::Write;

    //first we parse the pattern
    let mut pattern = vec![];
    let mut vars = vec![];
    let mut finish_or_fail = false;
    for (i, part) in str_pattern.split('}').enumerate() {
        if finish_or_fail {
            return ::std::result::Result::Err(FormatError::UnmatchedCurlyBracket);
        }
        if part.contains('{') {
            let mut part = part.split('{');
            let text = part.next().unwrap();
            let arg = part.next().ok_or(FormatError::UnmatchedCurlyBracket)?;
            if part.next() != ::std::option::Option::None {
                return ::std::result::Result::Err(FormatError::UnmatchedCurlyBracket);
            }
            pattern.push(text);
            vars.push(
                argv.get::<usize>(if arg.len() > 0 {
                    arg.parse()
                        .map_err(|_| FormatError::InvalidPositionalArgument)?
                } else {
                    i
                })
                .ok_or(FormatError::InvalidPositionalArgument)?,
            );
        } else {
            finish_or_fail = true;
            pattern.push(part);
        }
    }

    //then we generate the result String
    let mut res = ::std::string::String::with_capacity(str_pattern.len());
    let mut pattern = pattern.iter();
    let mut vars = vars.iter();
    while let ::std::option::Option::Some(text) = pattern.next() {
        res.write_str(text).unwrap();
        if let ::std::option::Option::Some(var) = vars.next() {
            res.write_str(&format!("{}", var)).unwrap();
        }
    }
    ::std::result::Result::Ok(res)
}


#[cfg(test)]
mod tests {
    #[test]
    fn basic_test() {
        assert_eq!(super::try_format("Hello {}", &[Box::new("world")]).unwrap(), "Hello world");
    }
}
