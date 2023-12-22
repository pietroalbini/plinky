use crate::parser::{GenericParam, Item};

pub(crate) fn generate_impl_for<T, F: FnOnce(&mut String) -> T>(
    output: &mut String,
    item: &Item,
    trait_: &str,
    f: F,
) -> T {
    let (name, generics) = match item {
        Item::Struct(s) => (&s.name, &s.generics),
        Item::Enum(e) => (&e.name, &e.generics),
    };

    output.push_str("impl");
    if !generics.is_empty() {
        output.push('<');
        for generic in generics {
            match generic {
                GenericParam::Normal(param) => {
                    output.push_str(&param.name);
                    output.push_str(": ");
                    output.push_str(&param.bound);
                }
                GenericParam::Const(param) => {
                    output.push_str("const ");
                    output.push_str(&param.name);
                    output.push_str(": ");
                    output.push_str(&param.type_);
                }
            }
            output.push(',');
        }
        output.push('>');
    }
    output.push(' ');
    output.push_str(trait_);
    output.push_str(" for ");
    output.push_str(name);
    if !generics.is_empty() {
        output.push('<');
        for generic in generics {
            output.push_str(match generic {
                GenericParam::Normal(param) => &param.name,
                GenericParam::Const(param) => &param.name,
            });
            output.push(',');
        }
        output.push('>');
    }
    output.push('{');
    let result = f(output);
    output.push('}');
    result
}
