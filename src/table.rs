use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColumnType {
    Text,
    Badge { color_map: Vec<(String, String)> },
    DateTime,
    Numeric,
    Image,
}

#[derive(Debug, Clone)]
pub struct Column {
    pub name: String,
    pub label: String,
    pub column_type: ColumnType,
    pub sortable: bool,
    pub searchable: bool,
    pub editable: bool,
    pub editable_options: Option<Vec<String>>,
}

impl Column {
    pub fn text(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            label: crate::resource::headline(&name),
            name,
            column_type: ColumnType::Text,
            sortable: false,
            searchable: false,
            editable: false,
            editable_options: None,
        }
    }

    pub fn numeric(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            label: crate::resource::headline(&name),
            name,
            column_type: ColumnType::Numeric,
            sortable: false,
            searchable: false,
            editable: false,
            editable_options: None,
        }
    }

    pub fn date_time(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            label: crate::resource::headline(&name),
            name,
            column_type: ColumnType::DateTime,
            sortable: false,
            searchable: false,
            editable: false,
            editable_options: None,
        }
    }

    pub fn image(name: impl Into<String>) -> Self {
        let name = name.into();
        Self {
            label: crate::resource::headline(&name),
            name,
            column_type: ColumnType::Image,
            sortable: false,
            searchable: false,
            editable: false,
            editable_options: None,
        }
    }

    pub fn badge(name: impl Into<String>, colors: Vec<(&str, &str)>) -> Self {
        let name = name.into();
        Self {
            label: crate::resource::headline(&name),
            name,
            column_type: ColumnType::Badge {
                color_map: colors.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            },
            sortable: false,
            searchable: false,
            editable: false,
            editable_options: None,
        }
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn sortable(mut self) -> Self {
        self.sortable = true;
        self
    }

    pub fn searchable(mut self) -> Self {
        self.searchable = true;
        self
    }

    pub fn editable(mut self) -> Self {
        self.editable = true;
        self
    }

    pub fn editable_select(mut self, options: Vec<impl Into<String>>) -> Self {
        self.editable = true;
        self.editable_options = Some(options.into_iter().map(|o| o.into()).collect());
        self
    }
}

/// Table Row Action
#[derive(Debug, Clone)]
pub struct Action {
    pub id: String,
    pub label: String,
    pub icon: Option<String>,
    pub color: String,
    pub requires_confirmation: bool,
    pub modal_heading: Option<String>,
    pub modal_description: Option<String>,
    pub action_url: Option<String>,
}

pub type TableAction = Action;

impl Action {
    pub fn make(id: impl Into<String>) -> Self {
        let id_str = id.into();
        let label = crate::resource::headline(&id_str);
        Self {
            id: id_str,
            label,
            icon: None,
            color: "zinc".to_string(),
            requires_confirmation: false,
            modal_heading: None,
            modal_description: None,
            action_url: None,
        }
    }

    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon: None,
            color: "zinc".to_string(),
            requires_confirmation: false,
            modal_heading: None,
            modal_description: None,
            action_url: None,
        }
    }

    /// Prebuilt View Action
    pub fn view() -> Self {
        Self::make("view")
            .label("View")
            .icon("eye")
            .color("zinc")
    }

    /// Prebuilt Edit Action
    pub fn edit() -> Self {
        Self::make("edit")
            .label("Edit")
            .icon("pencil")
            .color("zinc")
    }

    /// Prebuilt Delete Action
    pub fn delete() -> Self {
        Self::make("delete")
            .label("Delete")
            .icon("trash")
            .color("rose")
            .requires_confirmation(true)
            .modal_heading("Delete Record")
            .modal_description("Are you sure you want to delete this record? This action cannot be undone.")
    }

    /// Prebuilt Replicate Action (Duplicate record)
    pub fn replicate() -> Self {
        Self::make("replicate")
            .label("Duplicate")
            .icon("copy")
            .color("indigo")
            .requires_confirmation(true)
            .modal_heading("Duplicate Record")
            .modal_description("A duplicate of this record will be created with cloned attributes.")
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.color = color.into();
        self
    }

    pub fn danger(mut self) -> Self {
        self.color = "rose".to_string();
        self
    }

    pub fn confirm(mut self, message: impl Into<String>) -> Self {
        self.requires_confirmation = true;
        self.modal_description = Some(message.into());
        self
    }

    pub fn requires_confirmation(mut self, requires: bool) -> Self {
        self.requires_confirmation = requires;
        self
    }

    pub fn modal_heading(mut self, heading: impl Into<String>) -> Self {
        self.modal_heading = Some(heading.into());
        self
    }

    pub fn modal_description(mut self, desc: impl Into<String>) -> Self {
        self.modal_description = Some(desc.into());
        self
    }

    pub fn action_url(mut self, url: impl Into<String>) -> Self {
        self.action_url = Some(url.into());
        self
    }
}

/// Table Bulk Action
#[derive(Debug, Clone)]
pub struct BulkAction {
    pub id: String,
    pub label: String,
    pub icon: Option<String>,
    pub color: String,
    pub requires_confirmation: bool,
    pub modal_heading: Option<String>,
    pub modal_description: Option<String>,
}

impl BulkAction {
    pub fn make(id: impl Into<String>) -> Self {
        let id_str = id.into();
        let label = crate::resource::headline(&id_str);
        Self {
            id: id_str,
            label,
            icon: None,
            color: "zinc".to_string(),
            requires_confirmation: false,
            modal_heading: None,
            modal_description: None,
        }
    }

    /// Prebuilt Delete Bulk Action
    pub fn delete() -> Self {
        Self::make("delete")
            .label("Delete Selected")
            .icon("trash")
            .color("rose")
            .requires_confirmation(true)
            .modal_heading("Delete Selected Records")
            .modal_description("Are you sure you want to permanently delete all selected records?")
    }

    /// Prebuilt Streaming Export Bulk Action
    pub fn export() -> Self {
        Self::make("export")
            .label("Export CSV")
            .icon("download")
            .color("emerald")
    }

    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }

    pub fn color(mut self, color: impl Into<String>) -> Self {
        self.color = color.into();
        self
    }

    pub fn requires_confirmation(mut self, requires: bool) -> Self {
        self.requires_confirmation = requires;
        self
    }

    pub fn modal_heading(mut self, heading: impl Into<String>) -> Self {
        self.modal_heading = Some(heading.into());
        self
    }

    pub fn modal_description(mut self, desc: impl Into<String>) -> Self {
        self.modal_description = Some(desc.into());
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TableStyle {
    #[default]
    Default,
    Striped,
    Compact,
}

#[derive(Debug, Clone)]
pub struct TableFilter {
    pub name: String,
    pub label: String,
    pub options: Vec<(String, String)>,
}

impl TableFilter {
    pub fn select(name: impl Into<String>, label: impl Into<String>, options: Vec<(&str, &str)>) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            options: options.into_iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Table {
    pub columns: Vec<Column>,
    pub actions: Vec<Action>,
    pub bulk_actions: Vec<BulkAction>,
    pub filters: Vec<TableFilter>,
    pub style: TableStyle,
    pub default_sort_by: Option<String>,
    pub default_page_size: usize,
    pub empty_state_heading: Option<String>,
    pub empty_state_description: Option<String>,
    pub reorderable: bool,
}

impl Table {
    pub fn new() -> Self {
        Self {
            columns: Vec::new(),
            actions: Vec::new(),
            bulk_actions: Vec::new(),
            filters: Vec::new(),
            style: TableStyle::Default,
            default_sort_by: None,
            default_page_size: 10,
            empty_state_heading: None,
            empty_state_description: None,
            reorderable: false,
        }
    }

    pub fn reorderable(mut self) -> Self {
        self.reorderable = true;
        self
    }

    pub fn sortable_rows(self) -> Self {
        self.reorderable()
    }

    pub fn column(mut self, column: Column) -> Self {
        self.columns.push(column);
        self
    }

    pub fn columns(mut self, cols: Vec<Column>) -> Self {
        self.columns.extend(cols);
        self
    }

    pub fn action(mut self, action: Action) -> Self {
        self.actions.push(action);
        self
    }

    pub fn actions(mut self, actions: Vec<Action>) -> Self {
        self.actions = actions;
        self
    }

    pub fn bulk_action(mut self, action: BulkAction) -> Self {
        self.bulk_actions.push(action);
        self
    }

    pub fn bulk_actions(mut self, actions: Vec<BulkAction>) -> Self {
        self.bulk_actions = actions;
        self
    }

    pub fn filter(mut self, filter: TableFilter) -> Self {
        self.filters.push(filter);
        self
    }

    pub fn filters(mut self, filters: Vec<TableFilter>) -> Self {
        self.filters = filters;
        self
    }

    pub fn style(mut self, style: TableStyle) -> Self {
        self.style = style;
        self
    }

    pub fn striped(mut self) -> Self {
        self.style = TableStyle::Striped;
        self
    }

    pub fn compact(mut self) -> Self {
        self.style = TableStyle::Compact;
        self
    }

    pub fn page_size(mut self, size: usize) -> Self {
        self.default_page_size = size;
        self
    }

    pub fn default_sort_by(mut self, sort_by: impl Into<String>) -> Self {
        self.default_sort_by = Some(sort_by.into());
        self
    }

    pub fn empty_state_heading(mut self, heading: impl Into<String>) -> Self {
        self.empty_state_heading = Some(heading.into());
        self
    }

    pub fn empty_state_description(mut self, desc: impl Into<String>) -> Self {
        self.empty_state_description = Some(desc.into());
        self
    }
}

impl Default for Table {
    fn default() -> Self {
        Self::new()
    }
}
