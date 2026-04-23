use minijinja::{Environment, UndefinedBehavior};
use serde_json::{Map, Value};

use docli_core::DocliError;

pub fn render_template(content: &str, vars: &Map<String, Value>) -> Result<String, DocliError> {
    let mut environment = Environment::new();
    environment.set_undefined_behavior(UndefinedBehavior::Strict);
    environment
        .add_template("template", content)
        .map_err(|error| DocliError::InvalidSpec {
            message: error.to_string(),
        })?;

    let template =
        environment
            .get_template("template")
            .map_err(|error| DocliError::InvalidSpec {
                message: error.to_string(),
            })?;

    let mut context = vars.clone();
    context.insert("date".to_string(), Value::String("today".to_string()));
    template.render(Value::Object(context)).map_err(|error| {
        let message = error.to_string();
        if message.contains("undefined value") {
            DocliError::TemplateVarMissing { variable: message }
        } else {
            DocliError::InvalidSpec { message }
        }
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Map, Value};

    use super::render_template;

    #[test]
    fn renders_minijinja_templates_with_cli_vars() {
        let mut vars = Map::new();
        vars.insert("name".to_string(), Value::String("docli".to_string()));
        vars.insert("count".to_string(), Value::Number(3.into()));

        let rendered = render_template(
            "name: {{ name }}\ncount: {{ count }}\ndate: {{ date }}",
            &vars,
        )
        .unwrap();

        assert!(rendered.contains("name: docli"));
        assert!(rendered.contains("count: 3"));
        assert!(rendered.contains("date: today"));
    }
}
