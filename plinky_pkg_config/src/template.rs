use crate::parser::{ParseError, WhileResolving};
use std::collections::BTreeMap;

const MAX_TEMPLATE_SIZE: usize = 1024 * 1024 * 64; // 64MB

pub(crate) fn resolve_variables(
    templates: &mut BTreeMap<String, Template>,
) -> Result<BTreeMap<String, String>, ParseError> {
    // TODO: default variables???

    let mut result = BTreeMap::new();

    // First try to resolve variables in a loop until there are no variables that can be
    // resolved. This handles a variable depending on another variable without having a graph.
    loop {
        let mut this_cycle = 0;
        let mut to_remove = Vec::new();

        for (name, template) in templates.iter() {
            match template.resolve(&result, WhileResolving::Variable(name.clone())) {
                Ok(resolved) => {
                    result.insert(name.clone(), resolved);
                    to_remove.push(name.clone());
                    this_cycle += 1;
                }
                // Errors we ignore:
                Err(ParseError::UndefinedVariable(_, _)) => {}
                Err(err) => return Err(err),
            }
        }
        for name in to_remove {
            templates.remove(&name);
        }

        if this_cycle == 0 {
            break;
        }
    }

    // Then resolve the remaining variables without suppressing any error.
    while let Some((name, template)) = templates.pop_first() {
        result.insert(name.clone(), template.resolve(&result, WhileResolving::Variable(name))?);
    }

    Ok(result)
}

pub(crate) struct Template {
    pub(crate) components: Vec<TemplateComponent>,
}

impl Template {
    pub(crate) fn resolve(
        &self,
        variables: &BTreeMap<String, String>,
        while_resolving: WhileResolving,
    ) -> Result<String, ParseError> {
        let mut output = String::new();
        for component in &self.components {
            let new = match component {
                TemplateComponent::Text(text) => &*text,
                TemplateComponent::TextStatic(text) => *text,
                TemplateComponent::Variable(var) => &*variables.get(var).ok_or_else(|| {
                    ParseError::UndefinedVariable(var.clone(), while_resolving.clone())
                })?,
            };
            if output.len() + new.len() > MAX_TEMPLATE_SIZE {
                return Err(ParseError::ContentTooLarge(while_resolving.clone()));
            }
            output.push_str(new);
        }
        Ok(output)
    }
}

pub(crate) enum TemplateComponent {
    Text(String),
    TextStatic(&'static str),
    Variable(String),
}
