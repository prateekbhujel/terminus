use crate::form::{FieldType, Form};
use crate::resource::{FormMode, Resource};

pub fn render_dialogs(resource: &dyn Resource) -> String {
    let form = resource.form();
    let slug = resource.slug();
    let name = resource.name();
    let mode = resource.form_mode();

    let csrf_field = crate::helpers::csrf_field();
    let create_fields_html = render_form_fields(&form, "create");
    let edit_fields_html = render_form_fields(&form, "edit");

    // If resource uses FormMode::Page, create/edit forms are full pages, so only delete confirmation dialog is needed
    if mode == FormMode::Page {
        return render_delete_dialog(slug);
    }

    let (dialog_classes, edit_dialog_classes) = if mode == FormMode::SlideOver {
        (
            "fixed inset-y-0 right-0 h-full max-w-md w-full bg-zinc-900 border-l border-zinc-800 p-0 text-zinc-100 shadow-2xl backdrop:bg-black/70 backdrop:backdrop-blur-sm m-0 ml-auto flex flex-col",
            "fixed inset-y-0 right-0 h-full max-w-md w-full bg-zinc-900 border-l border-zinc-800 p-0 text-zinc-100 shadow-2xl backdrop:bg-black/70 backdrop:backdrop-blur-sm m-0 ml-auto flex flex-col"
        )
    } else {
        (
            "bg-zinc-900 border border-zinc-800 rounded-xl p-0 text-zinc-100 max-w-md w-full shadow-2xl backdrop:bg-black/75 backdrop:backdrop-blur-sm m-auto",
            "bg-zinc-900 border border-zinc-800 rounded-xl p-0 text-zinc-100 max-w-md w-full shadow-2xl backdrop:bg-black/75 backdrop:backdrop-blur-sm m-auto"
        )
    };

    let delete_dialog = render_delete_dialog(slug);

    format!(
        r#"<!-- Native In-App Create Dialog/SlideOver ({mode:?}) -->
<dialog id="create-dialog" class="{dialog_classes}">
    <div class="flex items-center justify-between px-5 py-4 border-b border-zinc-800 shrink-0">
        <h3 class="text-sm font-medium text-zinc-100">New {name}</h3>
        <button type="button" onclick="document.getElementById('create-dialog').close()" class="text-zinc-500 hover:text-zinc-300 text-lg leading-none">&times;</button>
    </div>
    <form method="POST" action="/admin/{slug}/create" class="p-5 space-y-4 flex-1 overflow-y-auto flex flex-col justify-between">
        {csrf_field}
        <div class="space-y-3.5">
            {create_fields_html}
        </div>
        <div class="pt-4 border-t border-zinc-800 flex items-center justify-end gap-2 shrink-0">
            <button type="button" onclick="document.getElementById('create-dialog').close()" class="px-3 py-1.5 rounded-lg border border-zinc-800 text-xs text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800 transition-colors">
                Cancel
            </button>
            <button type="submit" class="px-4 py-1.5 rounded-lg bg-zinc-100 hover:bg-white text-zinc-900 font-medium text-xs transition-colors shadow-sm">
                Create {name}
            </button>
        </div>
    </form>
</dialog>

<!-- Native In-App Edit Dialog/SlideOver ({mode:?}) -->
<dialog id="edit-dialog" class="{edit_dialog_classes}">
    <div class="flex items-center justify-between px-5 py-4 border-b border-zinc-800 shrink-0">
        <h3 id="edit-dialog-title" class="text-sm font-medium text-zinc-100">Edit {name}</h3>
        <button type="button" onclick="document.getElementById('edit-dialog').close()" class="text-zinc-500 hover:text-zinc-300 text-lg leading-none">&times;</button>
    </div>
    <form id="edit-form" method="POST" action="" class="p-5 space-y-4 flex-1 overflow-y-auto flex flex-col justify-between">
        {csrf_field}
        <div class="space-y-3.5">
            {edit_fields_html}
        </div>
        <div class="pt-4 border-t border-zinc-800 flex items-center justify-end gap-2 shrink-0">
            <button type="button" onclick="document.getElementById('edit-dialog').close()" class="px-3 py-1.5 rounded-lg border border-zinc-800 text-xs text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800 transition-colors">
                Cancel
            </button>
            <button type="submit" class="px-4 py-1.5 rounded-lg bg-zinc-100 hover:bg-white text-zinc-900 font-medium text-xs transition-colors shadow-sm">
                Save Changes
            </button>
        </div>
    </form>
</dialog>

{delete_dialog}"#,
        name = name,
        slug = slug,
        dialog_classes = dialog_classes,
        edit_dialog_classes = edit_dialog_classes,
        create_fields_html = create_fields_html,
        edit_fields_html = edit_fields_html,
        delete_dialog = delete_dialog,
        mode = mode
    )
}

fn render_delete_dialog(slug: &str) -> String {
    let csrf_field = crate::helpers::csrf_field();
    format!(
        r#"<!-- Native In-App Delete Confirmation Dialog -->
<dialog id="delete-dialog" class="bg-zinc-900 border border-zinc-800 rounded-xl p-0 text-zinc-100 max-w-sm w-full shadow-2xl backdrop:bg-black/75 backdrop:backdrop-blur-sm m-auto">
    <div class="p-5">
        <div class="flex items-start gap-3.5 mb-4">
            <div class="w-8 h-8 rounded-lg bg-rose-500/10 border border-rose-500/20 flex items-center justify-center shrink-0 text-rose-400">
                <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/><line x1="10" x2="10" y1="11" y2="17"/><line x1="14" x2="14" y1="11" y2="17"/></svg>
            </div>
            <div>
                <h4 class="text-sm font-medium text-zinc-100">Delete record</h4>
                <p class="text-xs text-zinc-400 mt-1">Are you sure you want to delete this record? This action cannot be undone.</p>
            </div>
        </div>
        <form id="delete-form" method="POST" action="/admin/{slug}/delete" class="flex items-center justify-end gap-2 pt-2 border-t border-zinc-800">
            {csrf_field}
            <button type="button" onclick="document.getElementById('delete-dialog').close()" class="px-3 py-1.5 rounded-lg border border-zinc-800 text-xs text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800 transition-colors">
                Cancel
            </button>
            <button type="submit" class="px-4 py-1.5 rounded-lg bg-rose-600 hover:bg-rose-500 text-white font-medium text-xs transition-colors shadow-sm">
                Delete
            </button>
        </form>
    </div>
</dialog>"#,
        slug = slug
    )
}

fn render_form_fields(form: &Form, prefix: &str) -> String {
    let mut out = String::new();

    for f in &form.fields {
        let input_id = format!("{prefix}-field-{}", f.name);
        let placeholder = f.placeholder.as_deref().unwrap_or("");
        let required = if f.required { "required" } else { "" };

        let field_input = match &f.field_type {
            FieldType::Select { options } => {
                let mut opts_html = String::new();
                for (val, label) in options {
                    opts_html.push_str(&format!(
                        r#"<option value="{val}">{label}</option>"#
                    ));
                }
                format!(
                    r#"<select id="{input_id}" name="{name}" {required} class="w-full px-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-xs text-zinc-200 focus:outline-none focus:border-zinc-600 transition-colors font-sans">{opts_html}</select>"#,
                    input_id = input_id,
                    name = f.name,
                    required = required,
                    opts_html = opts_html
                )
            }
            FieldType::SearchableSelect { options } => {
                let first_val = options.first().map(|(v, _)| v.as_str()).unwrap_or("");
                let first_lbl = options.first().map(|(_, l)| l.as_str()).unwrap_or("Select...");
                let mut items_html = String::new();
                for (val, label) in options {
                    items_html.push_str(&format!(
                        r#"<div onclick="selectSearchableOption('{input_id}', '{val}', '{label}')" data-label="{label}" class="searchable-opt px-2.5 py-1.5 text-xs text-zinc-300 hover:bg-zinc-800 hover:text-white rounded cursor-pointer transition-colors flex items-center justify-between"><span>{label}</span><span class="text-[10px] text-zinc-500 font-mono">{val}</span></div>"#
                    ));
                }
                format!(
                    r#"<div class="relative" id="container-{input_id}">
                        <input type="hidden" id="{input_id}" name="{name}" value="{first_val}" {required} />
                        <button type="button" onclick="toggleSearchableSelect('{input_id}')" id="btn-{input_id}" class="w-full flex items-center justify-between px-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-xs text-zinc-200 focus:outline-none focus:border-zinc-600 transition-colors">
                            <span id="label-{input_id}" class="truncate">{first_lbl}</span>
                            <svg class="w-3.5 h-3.5 text-zinc-500 shrink-0 ml-2" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"></path></svg>
                        </button>
                        <div id="dropdown-{input_id}" class="hidden absolute z-50 left-0 right-0 mt-1 bg-zinc-900 border border-zinc-800 rounded-lg shadow-2xl p-1.5 max-h-52 overflow-y-auto backdrop-blur-md">
                            <div class="p-1 border-b border-zinc-800 mb-1">
                                <input type="text" placeholder="Search options..." oninput="filterSearchableSelect('{input_id}', this.value)" class="w-full px-2 py-1 bg-zinc-950 border border-zinc-800 rounded text-xs text-zinc-200 placeholder-zinc-500 focus:outline-none focus:border-zinc-600 font-sans" />
                            </div>
                            <div class="space-y-0.5" id="opts-{input_id}">
                                {items_html}
                            </div>
                        </div>
                    </div>"#,
                    input_id = input_id,
                    name = f.name,
                    required = required,
                    first_val = first_val,
                    first_lbl = first_lbl,
                    items_html = items_html
                )
            }
            FieldType::Email => {
                format!(
                    r#"<input type="email" id="{input_id}" name="{name}" placeholder="{placeholder}" {required} class="w-full px-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-zinc-600 transition-colors" />"#,
                    input_id = input_id,
                    name = f.name,
                    placeholder = placeholder,
                    required = required
                )
            }
            FieldType::Textarea => {
                format!(
                    r#"<textarea id="{input_id}" name="{name}" rows="{rows}" placeholder="{placeholder}" {required} class="w-full px-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-zinc-600 transition-colors"></textarea>"#,
                    input_id = input_id,
                    name = f.name,
                    rows = f.rows.unwrap_or(3),
                    placeholder = placeholder,
                    required = required
                )
            }
            FieldType::RichText => {
                format!(
                    r#"<div class="space-y-1" id="wrapper-{input_id}">
                        <div class="flex items-center gap-1 p-1 bg-zinc-900 border border-zinc-800 rounded-t-lg text-zinc-400">
                            <button type="button" onclick="formatRichText('{input_id}', '**', '**')" class="px-2 py-0.5 rounded text-[11px] font-bold hover:bg-zinc-800 hover:text-white transition-colors" title="Bold">B</button>
                            <button type="button" onclick="formatRichText('{input_id}', '*', '*')" class="px-2 py-0.5 rounded text-[11px] italic hover:bg-zinc-800 hover:text-white transition-colors" title="Italic">I</button>
                            <button type="button" onclick="formatRichText('{input_id}', '## ', '')" class="px-2 py-0.5 rounded text-[11px] font-semibold hover:bg-zinc-800 hover:text-white transition-colors" title="H2">H2</button>
                            <button type="button" onclick="formatRichText('{input_id}', '### ', '')" class="px-2 py-0.5 rounded text-[11px] font-semibold hover:bg-zinc-800 hover:text-white transition-colors" title="H3">H3</button>
                            <button type="button" onclick="formatRichText('{input_id}', '- ', '')" class="px-2 py-0.5 rounded text-[11px] hover:bg-zinc-800 hover:text-white transition-colors" title="Bullet List">• List</button>
                            <button type="button" onclick="formatRichText('{input_id}', '> ', '')" class="px-2 py-0.5 rounded text-[11px] hover:bg-zinc-800 hover:text-white transition-colors" title="Quote">“ Quote</button>
                            <button type="button" onclick="formatRichText('{input_id}', '`', '`')" class="px-2 py-0.5 rounded text-[11px] font-mono hover:bg-zinc-800 hover:text-white transition-colors" title="Code">&lt;&gt;</button>
                        </div>
                        <textarea id="{input_id}" name="{name}" rows="{rows}" placeholder="{placeholder}" {required} class="w-full px-3 py-2 bg-zinc-950 border border-t-0 border-zinc-800 rounded-b-lg text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-zinc-600 font-mono transition-colors"></textarea>
                    </div>"#,
                    input_id = input_id,
                    name = f.name,
                    rows = f.rows.unwrap_or(4),
                    placeholder = placeholder,
                    required = required
                )
            }
            FieldType::Image | FieldType::Gallery => {
                format!(
                    r#"<div class="space-y-2">
                        <input type="text" id="{input_id}" name="{name}" placeholder="Image URL (https://...)" oninput="updateImagePreview('{input_id}', this.value)" {required} class="w-full px-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-zinc-600 transition-colors" />
                        <div id="preview-{input_id}" class="flex items-center gap-2.5 p-2 bg-zinc-900/60 border border-zinc-800 rounded-lg">
                            <img id="img-{input_id}" src="https://placehold.co/128x128/27272a/71717a?text=Preview" alt="Preview" class="w-12 h-12 rounded object-cover border border-zinc-800 bg-zinc-950 shrink-0" onerror="this.src='https://placehold.co/128x128/27272a/71717a?text=Preview'" />
                            <span class="text-[11px] text-zinc-400">Media preview & thumbnail inspection</span>
                        </div>
                    </div>"#,
                    input_id = input_id,
                    name = f.name,
                    required = required
                )
            }
            FieldType::Barcode => {
                format!(
                    r#"<div class="relative">
                        <div class="absolute inset-y-0 left-0 pl-2.5 flex items-center pointer-events-none text-zinc-500">
                            <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 5v14M8 5v14M12 5v14M17 5v14M21 5v14"/></svg>
                        </div>
                        <input type="text" id="{input_id}" name="{name}" placeholder="{placeholder}" {required} class="w-full pl-8 pr-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-xs text-zinc-200 font-mono placeholder-zinc-600 focus:outline-none focus:border-emerald-500 focus:ring-1 focus:ring-emerald-500 transition-colors" />
                    </div>"#,
                    input_id = input_id,
                    name = f.name,
                    placeholder = if placeholder.is_empty() { "Scan barcode or enter SKU..." } else { placeholder },
                    required = required
                )
            }
            FieldType::Color => {
                format!(
                    r##"<div class="flex items-center gap-2">
                        <input type="color" id="picker-{input_id}" value="#18181b" onchange="document.getElementById('{input_id}').value = this.value" class="w-7 h-7 rounded border border-zinc-800 bg-zinc-950 cursor-pointer p-0.5 shrink-0" />
                        <input type="text" id="{input_id}" name="{name}" placeholder="#000000" oninput="document.getElementById('picker-{input_id}').value = this.value" {required} class="w-full px-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-xs text-zinc-200 font-mono placeholder-zinc-600 focus:outline-none focus:border-zinc-600 transition-colors" />
                    </div>"##,
                    input_id = input_id,
                    name = f.name,
                    required = required
                )
            }
            _ => {
                format!(
                    r#"<input type="text" id="{input_id}" name="{name}" placeholder="{placeholder}" {required} class="w-full px-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-zinc-600 transition-colors" />"#,
                    input_id = input_id,
                    name = f.name,
                    placeholder = placeholder,
                    required = required
                )
            }
        };

        out.push_str(&format!(
            r#"<div>
                <label for="{input_id}" class="block text-xs font-medium text-zinc-300 mb-1.5">{label}</label>
                {field_input}
            </div>"#,
            input_id = input_id,
            label = f.label,
            field_input = field_input
        ));
    }

    out
}
