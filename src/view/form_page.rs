use std::collections::HashMap;
use crate::form::{FieldType, Form};
use crate::resource::Resource;

pub fn render_form_page(
    resource: &dyn Resource,
    id: Option<&str>,
    values: Option<&HashMap<String, String>>,
) -> String {
    let form = resource.form();
    let slug = resource.slug();
    let name = resource.name();
    let is_edit = id.is_some();

    let (title, form_action) = if let Some(rec_id) = id {
        (format!("Edit {name} #{rec_id}"), format!("/admin/{slug}/edit/{rec_id}"))
    } else {
        (format!("Create {name}"), format!("/admin/{slug}/create"))
    };

    let fields_html = render_page_fields(&form, values);
    let csrf_field = crate::helpers::csrf_field();

    format!(
        r#"<div class="max-w-2xl mx-auto space-y-6">
            <!-- Header with Back Link -->
            <div class="flex items-center justify-between">
                <div>
                    <a href="/admin/{slug}" class="text-xs text-zinc-500 hover:text-zinc-300 flex items-center gap-1.5 transition-colors mb-2">
                        <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7"></path></svg>
                        <span>Back to {plural}</span>
                    </a>
                    <h1 class="text-xl font-semibold tracking-tight text-zinc-100">{title}</h1>
                </div>
            </div>

            <!-- Form Card -->
            <div class="bg-zinc-900 border border-zinc-800 rounded-xl p-6 shadow-sm">
                <form method="POST" action="{form_action}" class="space-y-5">
                    {csrf_field}
                    <div class="space-y-4">
                        {fields_html}
                    </div>

                    <div class="pt-5 border-t border-zinc-800 flex items-center justify-end gap-3">
                        <a href="/admin/{slug}" class="px-3.5 py-2 rounded-lg border border-zinc-800 text-xs font-medium text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800 transition-colors">
                            Cancel
                        </a>
                        <button type="submit" class="px-5 py-2 rounded-lg bg-zinc-100 hover:bg-white text-zinc-900 font-medium text-xs transition-colors shadow-sm">
                            {btn_text}
                        </button>
                    </div>
                </form>
            </div>
        </div>"#,
        slug = slug,
        plural = resource.plural_name(),
        title = title,
        form_action = form_action,
        fields_html = fields_html,
        btn_text = if is_edit { "Save Changes" } else { "Create Record" }
    )
}

fn render_page_fields(form: &Form, values: Option<&HashMap<String, String>>) -> String {
    let mut out = String::new();

    for f in &form.fields {
        let input_id = format!("page-field-{}", f.name);
        let placeholder = f.placeholder.as_deref().unwrap_or("");
        let required = if f.required { "required" } else { "" };
        let current_val = values
            .and_then(|v| {
                v.get(&f.name)
                    .or_else(|| v.get(&crate::resource::to_camel_case(&f.name)))
                    .or_else(|| v.get(&crate::resource::to_snake_case(&f.name)))
            })
            .map(|s| s.as_str())
            .unwrap_or("");

        let field_input = match &f.field_type {
            FieldType::Select { options } => {
                let mut opts_html = String::new();
                for (val, label) in options {
                    let sel = if val == current_val { "selected" } else { "" };
                    opts_html.push_str(&format!(
                        r#"<option value="{val}" {sel}>{label}</option>"#
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
                let active_val = if !current_val.is_empty() {
                    current_val
                } else {
                    options.first().map(|(v, _)| v.as_str()).unwrap_or("")
                };
                let active_lbl = options.iter().find(|(v, _)| v == active_val).map(|(_, l)| l.as_str()).unwrap_or(active_val);

                let mut items_html = String::new();
                for (val, label) in options {
                    items_html.push_str(&format!(
                        r#"<div onclick="selectSearchableOption('{input_id}', '{val}', '{label}')" data-label="{label}" class="searchable-opt px-2.5 py-1.5 text-xs text-zinc-300 hover:bg-zinc-800 hover:text-white rounded cursor-pointer transition-colors flex items-center justify-between"><span>{label}</span><span class="text-[10px] text-zinc-500 font-mono">{val}</span></div>"#
                    ));
                }
                format!(
                    r#"<div class="relative" id="container-{input_id}">
                        <input type="hidden" id="{input_id}" name="{name}" value="{active_val}" {required} />
                        <button type="button" onclick="toggleSearchableSelect('{input_id}')" id="btn-{input_id}" class="w-full flex items-center justify-between px-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-xs text-zinc-200 focus:outline-none focus:border-zinc-600 transition-colors">
                            <span id="label-{input_id}" class="truncate">{active_lbl}</span>
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
                    active_val = active_val,
                    active_lbl = active_lbl,
                    items_html = items_html
                )
            }
            FieldType::Email => {
                format!(
                    r#"<input type="email" id="{input_id}" name="{name}" value="{current_val}" placeholder="{placeholder}" {required} class="w-full px-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-zinc-600 transition-colors" />"#,
                    input_id = input_id,
                    name = f.name,
                    current_val = current_val,
                    placeholder = placeholder,
                    required = required
                )
            }
            FieldType::Textarea => {
                format!(
                    r#"<textarea id="{input_id}" name="{name}" rows="{rows}" placeholder="{placeholder}" {required} class="w-full px-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-zinc-600 transition-colors font-sans">{current_val}</textarea>"#,
                    input_id = input_id,
                    name = f.name,
                    rows = f.rows.unwrap_or(3),
                    placeholder = placeholder,
                    required = required,
                    current_val = current_val
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
                        <textarea id="{input_id}" name="{name}" rows="{rows}" placeholder="{placeholder}" {required} class="w-full px-3 py-2 bg-zinc-950 border border-t-0 border-zinc-800 rounded-b-lg text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-zinc-600 font-mono transition-colors">{current_val}</textarea>
                    </div>"#,
                    input_id = input_id,
                    name = f.name,
                    rows = f.rows.unwrap_or(5),
                    placeholder = placeholder,
                    required = required,
                    current_val = current_val
                )
            }
            FieldType::Image | FieldType::Gallery => {
                let img_src = if current_val.is_empty() {
                    "https://placehold.co/128x128/27272a/71717a?text=Preview"
                } else {
                    current_val
                };
                format!(
                    r#"<div class="space-y-2">
                        <input type="text" id="{input_id}" name="{name}" value="{current_val}" placeholder="Image URL (https://...)" oninput="updateImagePreview('{input_id}', this.value)" {required} class="w-full px-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-zinc-600 transition-colors" />
                        <div id="preview-{input_id}" class="flex items-center gap-2.5 p-2 bg-zinc-900/60 border border-zinc-800 rounded-lg">
                            <img id="img-{input_id}" src="{img_src}" alt="Preview" class="w-12 h-12 rounded object-cover border border-zinc-800 bg-zinc-950 shrink-0" onerror="this.src='https://placehold.co/128x128/27272a/71717a?text=Preview'" />
                            <span class="text-[11px] text-zinc-400">Media preview & thumbnail inspection</span>
                        </div>
                    </div>"#,
                    input_id = input_id,
                    name = f.name,
                    current_val = current_val,
                    img_src = img_src,
                    required = required
                )
            }
            FieldType::Barcode => {
                format!(
                    r#"<div class="relative">
                        <div class="absolute inset-y-0 left-0 pl-2.5 flex items-center pointer-events-none text-zinc-500">
                            <svg class="w-3.5 h-3.5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 5v14M8 5v14M12 5v14M17 5v14M21 5v14"/></svg>
                        </div>
                        <input type="text" id="{input_id}" name="{name}" value="{current_val}" placeholder="{placeholder}" {required} class="w-full pl-8 pr-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-xs text-zinc-200 font-mono placeholder-zinc-600 focus:outline-none focus:border-emerald-500 focus:ring-1 focus:ring-emerald-500 transition-colors" />
                    </div>"#,
                    input_id = input_id,
                    name = f.name,
                    current_val = current_val,
                    placeholder = if placeholder.is_empty() { "Scan barcode or enter SKU..." } else { placeholder },
                    required = required
                )
            }
            FieldType::Color => {
                let color_val = if current_val.is_empty() { "#18181b" } else { current_val };
                format!(
                    r##"<div class="flex items-center gap-2">
                        <input type="color" id="picker-{input_id}" value="{color_val}" onchange="document.getElementById('{input_id}').value = this.value" class="w-7 h-7 rounded border border-zinc-800 bg-zinc-950 cursor-pointer p-0.5 shrink-0" />
                        <input type="text" id="{input_id}" name="{name}" value="{current_val}" placeholder="#000000" oninput="document.getElementById('picker-{input_id}').value = this.value" {required} class="w-full px-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-xs text-zinc-200 font-mono placeholder-zinc-600 focus:outline-none focus:border-zinc-600 transition-colors" />
                    </div>"##,
                    input_id = input_id,
                    name = f.name,
                    color_val = color_val,
                    current_val = current_val,
                    required = required
                )
            }
            _ => {
                format!(
                    r#"<input type="text" id="{input_id}" name="{name}" value="{current_val}" placeholder="{placeholder}" {required} class="w-full px-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-zinc-600 transition-colors" />"#,
                    input_id = input_id,
                    name = f.name,
                    current_val = current_val,
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
