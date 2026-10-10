//! Status of monitored processes and the label texts that show it.

use serde::Deserialize;

/// Status of a monitored process. Always one of three values; the user only renames them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Exactly one matching process runs.
    On,
    /// No matching process runs.
    Off,
    /// The process list could not be read, or more than one matching process runs.
    Fault,
}

impl Status {
    /// Maps the number of matching processes (`None` if the check failed) to a status.
    pub fn from_count(count: Option<usize>) -> Self {
        match count {
            Some(0) => Status::Off,
            Some(1) => Status::On,
            Some(_) | None => Status::Fault,
        }
    }
}

/// Display texts of the three states.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusTexts {
    pub on: String,
    pub off: String,
    pub fault: String,
}

impl Default for StatusTexts {
    fn default() -> Self {
        Self {
            on: "On".into(),
            off: "Off".into(),
            fault: "Fault".into(),
        }
    }
}

impl StatusTexts {
    /// Returns these texts with the states given in `overrides` replaced.
    pub fn merged(&self, overrides: &StatusTextOverrides) -> Self {
        Self {
            on: overrides.on.clone().unwrap_or_else(|| self.on.clone()),
            off: overrides.off.clone().unwrap_or_else(|| self.off.clone()),
            fault: overrides
                .fault
                .clone()
                .unwrap_or_else(|| self.fault.clone()),
        }
    }

    pub fn get(&self, status: Status) -> &str {
        match status {
            Status::On => &self.on,
            Status::Off => &self.off,
            Status::Fault => &self.fault,
        }
    }
}

/// Partial override of [`StatusTexts`] as written in the YAML file.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StatusTextOverrides {
    pub on: Option<String>,
    pub off: Option<String>,
    pub fault: Option<String>,
}

/// A label text split into literal text and placeholders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template(Vec<Segment>);

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Text(String),
    Status,
}

impl Template {
    /// Parses `{STATUS}` placeholders; `{{` and `}}` stand for literal braces.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut segments = Vec::new();
        let mut literal = String::new();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '{' if chars.peek() == Some(&'{') => {
                    chars.next();
                    literal.push('{');
                }
                '}' if chars.peek() == Some(&'}') => {
                    chars.next();
                    literal.push('}');
                }
                '{' => {
                    let mut name = String::new();
                    loop {
                        match chars.next() {
                            Some('}') => break,
                            Some(c) => name.push(c),
                            None => return Err(format!("unterminated placeholder {{{name}")),
                        }
                    }
                    match name.as_str() {
                        "STATUS" => {
                            if !literal.is_empty() {
                                segments.push(Segment::Text(std::mem::take(&mut literal)));
                            }
                            segments.push(Segment::Status);
                        }
                        _ => {
                            return Err(format!(
                                "unknown placeholder {{{name}}} (supported: {{STATUS}}, write {{{{ and }}}} for literal braces)"
                            ));
                        }
                    }
                }
                '}' => return Err("unmatched '}' (write }} for a literal brace)".into()),
                c => literal.push(c),
            }
        }
        if !literal.is_empty() {
            segments.push(Segment::Text(literal));
        }
        Ok(Self(segments))
    }

    pub fn has_status(&self) -> bool {
        self.0.contains(&Segment::Status)
    }

    /// Renders the text; `status` replaces `{STATUS}` (empty if `None`).
    pub fn render(&self, status: Option<&str>) -> String {
        self.0
            .iter()
            .map(|segment| match segment {
                Segment::Text(text) => text.as_str(),
                Segment::Status => status.unwrap_or_default(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_from_count() {
        assert_eq!(Status::from_count(Some(0)), Status::Off);
        assert_eq!(Status::from_count(Some(1)), Status::On);
        assert_eq!(Status::from_count(Some(2)), Status::Fault);
        assert_eq!(Status::from_count(None), Status::Fault);
    }

    #[test]
    fn partial_overrides_keep_other_texts() {
        let global = StatusTexts::default().merged(&StatusTextOverrides {
            fault: Some("Error".into()),
            ..Default::default()
        });
        let label = global.merged(&StatusTextOverrides {
            on: Some("Yes".into()),
            ..Default::default()
        });
        assert_eq!(label.get(Status::On), "Yes");
        assert_eq!(label.get(Status::Off), "Off");
        assert_eq!(label.get(Status::Fault), "Error");
    }

    #[test]
    fn template_renders_status() {
        let t = Template::parse("Driver: {STATUS}!").unwrap();
        assert!(t.has_status());
        assert_eq!(t.render(Some("On")), "Driver: On!");
        assert_eq!(t.render(None), "Driver: !");
    }

    #[test]
    fn template_escapes_and_plain_text() {
        let t = Template::parse("{{STATUS}} {STATUS}{STATUS}").unwrap();
        assert_eq!(t.render(Some("x")), "{STATUS} xx");
        let plain = Template::parse("no placeholder").unwrap();
        assert!(!plain.has_status());
        assert_eq!(plain.render(Some("x")), "no placeholder");
    }

    #[test]
    fn template_rejects_unknown_and_unmatched() {
        assert!(
            Template::parse("{status}")
                .unwrap_err()
                .contains("{status}")
        );
        assert!(Template::parse("a } b").is_err());
        assert!(Template::parse("{STATUS").is_err());
    }
}
