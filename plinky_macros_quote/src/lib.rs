use proc_macro::{Delimiter, Ident, Literal, Punct, Spacing, TokenStream, TokenTree};

#[proc_macro]
pub fn quote(tokens: TokenStream) -> TokenStream {
    let nodes = parse(tokens);

    let mut output = String::new();
    render_node_list(&mut output, nodes);
    output.parse().unwrap()
}

fn parse(stream: TokenStream) -> Vec<Node> {
    let mut result = Vec::new();

    let mut iter = stream.into_iter().peekable();
    while let Some(token) = iter.next() {
        result.push(match token {
            TokenTree::Punct(pound) if pound.as_char() == '#' => {
                match iter.peek() {
                    Some(TokenTree::Ident(var)) => {
                        let var = var.to_string();
                        let _ = iter.next(); // Consume the peeked token.
                        Node::Interpolation(var)
                    }
                    Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace => {
                        let var = group.to_string();
                        let _ = iter.next(); // Consume the peeked token.
                        Node::Interpolation(var)
                    }
                    _ => Node::Punct(pound),
                }
            }

            TokenTree::Punct(punct) => Node::Punct(punct),
            TokenTree::Ident(ident) => Node::Ident(ident),
            TokenTree::Group(group) => Node::Group(group.delimiter(), parse(group.stream())),
            TokenTree::Literal(literal) => Node::Literal(literal),
        });
    }

    result
}

fn render_node_list(output: &mut String, nodes: Vec<Node>) {
    output.push('{');
    output.push_str("let mut __quote_buffer__ = proc_macro::TokenStream::new();");
    for node in nodes {
        match node {
            Node::Group(delimiter, subnodes) => {
                output.push_str(
                    "__quote_buffer__.extend([proc_macro::TokenTree::Group(proc_macro::Group::new(",
                );
                match delimiter {
                    Delimiter::Parenthesis => output.push_str("proc_macro::Delimiter::Parenthesis"),
                    Delimiter::Brace => output.push_str("proc_macro::Delimiter::Brace"),
                    Delimiter::Bracket => output.push_str("proc_macro::Delimiter::Bracket"),
                    Delimiter::None => output.push_str("proc_macro::Delimiter::None"),
                }
                output.push(',');
                render_node_list(output, subnodes);
                output.push_str("))]);");
            }
            Node::Ident(ident) => {
                output.push_str("__quote_buffer__.extend([proc_macro::TokenTree::Ident(proc_macro::Ident::new(\"");
                output.push_str(&ident.to_string());
                output.push_str("\", proc_macro::Span::call_site()))]);");
            }
            Node::Literal(literal) => {
                // Literals are round tripped into a string literal to take care of quoting.
                let quoted = Literal::string(&literal.to_string()).to_string();

                output.push_str("__quote_buffer__.extend([proc_macro::TokenTree::Literal(");
                output.push_str(&quoted);
                output.push_str(".parse().unwrap())]);");
            }
            Node::Punct(punct) => {
                output.push_str(
                    "__quote_buffer__.extend([proc_macro::TokenTree::Punct(proc_macro::Punct::new(",
                );
                output.push_str(&format!("{:?}", punct.as_char()));
                output.push(',');
                match punct.spacing() {
                    Spacing::Joint => output.push_str("proc_macro::Spacing::Joint"),
                    Spacing::Alone => output.push_str("proc_macro::Spacing::Alone"),
                }
                output.push_str("))]);");
            }
            Node::Interpolation(var) => {
                output.push_str(
                    "__quote_buffer__.extend(plinky_utils::quote::Quote::to_token_stream(&",
                );
                output.push_str(&var);
                output.push_str("));");
            }
        }
    }
    output.push_str("__quote_buffer__");
    output.push('}');
}

#[derive(Debug)]
enum Node {
    Group(Delimiter, Vec<Node>),
    Ident(Ident),
    Literal(Literal),
    Punct(Punct),
    Interpolation(String),
}
