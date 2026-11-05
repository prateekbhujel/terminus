use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FieldType {
    Text,
    Email,
    Password,
    Number,
    Select { options: Vec<(String, String)> },
    SearchableSelect { options: Vec<(String, String)> },
    Textarea,
    Toggle,
    DateTime,
    RichText,
    Image,
    Gallery,
    Barcode,
    Color,
    KeyValue,
    DraggableBlocks { block_types: Vec<String> },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    pub name: String,
    pub label: String,
    pub field_type: FieldType,
    pub required: bool,
    pub placeholder: Option<String>,
    pub helper_text: Option<String>,
    pub prefix: Option<String>,
    pub suffix: Option<String>,
    pub default_value: Option<String>,
    pub column_span: usize,
    pub disabled: bool,
    pub read_only: bool,
    pub min: Option<String>,
    pub max: Option<String>,
    pub step: Option<String>,
    pub rows: Option<usize>,
}

impl FormField {
    fn new_field(name: impl Into<String>, field_type: FieldType) -> Self {
        let name = name.into();
        Self {
            label: crate::resource::headline(&name),
            name,
            field_type,
            required: false,
            placeholder: None,
            helper_text: None,
            prefix: None,
            suffix: None,
            default_value: None,
            column_span: 1,
            disabled: false,
            read_only: false,
            min: None,
            max: None,
            step: None,
            rows: None,
        }
    }
    pub fn text(name: impl Into<String>) -> Self {
        Self::new_field(name, FieldType::Text)
    }

    pub fn email(name: impl Into<String>) -> Self {
        Self::new_field(name, FieldType::Email)
    }

    pub fn password(name: impl Into<String>) -> Self {
        Self::new_field(name, FieldType::Password)
    }

    pub fn number(name: impl Into<String>) -> Self {
        Self::new_field(name, FieldType::Number)
    }

    pub fn textarea(name: impl Into<String>) -> Self {
        Self::new_field(name, FieldType::Textarea)
    }

    pub fn toggle(name: impl Into<String>) -> Self {
        Self::new_field(name, FieldType::Toggle)
    }

    pub fn date_time(name: impl Into<String>) -> Self {
        Self::new_field(name, FieldType::DateTime)
    }

    pub fn datetime(name: impl Into<String>) -> Self {
        Self::date_time(name)
    }

    pub fn select(name: impl Into<String>, options: Vec<(&str, &str)>) -> Self {
        Self::new_field(
            name,
            FieldType::Select {
                options: options.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            },
        )
    }

    pub fn searchable_select(name: impl Into<String>, options: Vec<(&str, &str)>) -> Self {
        Self::new_field(
            name,
            FieldType::SearchableSelect {
                options: options.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            },
        )
    }

    pub fn rich_text(name: impl Into<String>) -> Self {
        Self::new_field(name, FieldType::RichText)
    }

    pub fn image(name: impl Into<String>) -> Self {
        Self::new_field(name, FieldType::Image)
    }

    pub fn gallery(name: impl Into<String>) -> Self {
        Self::new_field(name, FieldType::Gallery)
    }

    pub fn barcode(name: impl Into<String>) -> Self {
        Self::new_field(name, FieldType::Barcode)
    }

    pub fn color(name: impl Into<String>) -> Self {
        Self::new_field(name, FieldType::Color)
    }

    pub fn key_value(name: impl Into<String>) -> Self {
        Self::new_field(name, FieldType::KeyValue)
    }

    pub fn draggable_blocks(name: impl Into<String>, block_types: Vec<impl Into<String>>) -> Self {
        Self::new_field(
            name,
            FieldType::DraggableBlocks {
                block_types: block_types.into_iter().map(|b| b.into()).collect(),
            },
        )
    }

    pub fn required(mut self) -> Self {
        self.required = true;
        self
    }

    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = Some(text.into());
        self
    }

    pub fn helper_text(mut self, text: impl Into<String>) -> Self {
        self.helper_text = Some(text.into());
        self
    }

    pub fn prefix(mut self, p: impl Into<String>) -> Self {
        self.prefix = Some(p.into());
        self
    }

    pub fn suffix(mut self, s: impl Into<String>) -> Self {
        self.suffix = Some(s.into());
        self
    }

    pub fn default_value(mut self, val: impl Into<String>) -> Self {
        self.default_value = Some(val.into());
        self
    }

    pub fn column_span(mut self, span: usize) -> Self {
        self.column_span = span;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.disabled = true;
        self
    }

    pub fn read_only(mut self) -> Self {
        self.read_only = true;
        self
    }

    pub fn min(mut self, min: impl Into<String>) -> Self {
        self.min = Some(min.into());
        self
    }

    pub fn max(mut self, max: impl Into<String>) -> Self {
        self.max = Some(max.into());
        self
    }

    pub fn step(mut self, step: impl Into<String>) -> Self {
        self.step = Some(step.into());
        self
    }

    pub fn rows(mut self, rows: usize) -> Self {
        self.rows = Some(rows);
        self
    }
}

/// Form Section component
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormSection {
    pub heading: String,
    pub description: Option<String>,
    pub fields: Vec<FormField>,
    pub columns: usize,
}

pub type Section = FormSection;

impl FormSection {
    pub fn make(heading: impl Into<String>) -> Self {
        Self {
            heading: heading.into(),
            description: None,
            fields: Vec::new(),
            columns: 2,
        }
    }

    pub fn title(&self) -> &str {
        &self.heading
    }

    pub fn description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    pub fn schema(mut self, fields: Vec<FormField>) -> Self {
        self.fields = fields;
        self
    }

    pub fn columns(mut self, count: usize) -> Self {
        self.columns = count;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct Form {
    pub fields: Vec<FormField>,
    pub sections: Vec<FormSection>,
    pub columns: usize,
}

impl Form {
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            sections: Vec::new(),
            columns: 2,
        }
    }

    pub fn schema(mut self, fields: Vec<FormField>) -> Self {
        self.fields = fields;
        self
    }

    pub fn field(mut self, field: FormField) -> Self {
        self.fields.push(field);
        self
    }

    pub fn section(mut self, section: FormSection) -> Self {
        self.sections.push(section);
        self
    }

    pub fn sections(mut self, sections: Vec<FormSection>) -> Self {
        self.sections = sections;
        self
    }

    pub fn columns(mut self, count: usize) -> Self {
        self.columns = count;
        self
    }
}
