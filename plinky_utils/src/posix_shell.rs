// https://pubs.opengroup.org/onlinepubs/9699919799/utilities/V3_chap02.html#tag_18_02
pub fn posix_shell_quote(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    for chr in input.chars() {
        match chr {
            '|' | '&' | ';' | '<' | '>' | '(' | ')' | '$' | '`' | '\\' | '"' | '\'' | '*' | '?'
            | '~' | ' ' | '\t' => {
                result.push('\\');
                result.push(chr);
            }
            '\n' => {
                result.push('\'');
                result.push(chr);
                result.push('\'');
            }
            chr => result.push(chr),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_posix_shell_quote() {
        assert_eq!(r#"Hello"#, posix_shell_quote(r#"Hello"#));
        assert_eq!(r#"Hello\ world"#, posix_shell_quote(r#"Hello world"#));
        assert_eq!("Hello'\n'world", posix_shell_quote("Hello\nworld"));
    }
}
