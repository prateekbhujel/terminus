use crate::resource::DynResource;
use crate::theme::ThemeConfig;

pub fn render_page(
    title: &str,
    current_slug: &str,
    resources: &[DynResource],
    content_html: &str,
    dialogs_html: &str,
    flash_message: Option<&str>,
    user: &crate::domain::User,
    theme: &ThemeConfig,
) -> String {
    let is_dash_active = current_slug == "dashboard" || current_slug.is_empty();
    let dash_classes = if is_dash_active {
        "bg-zinc-800 text-zinc-100 font-medium border-l-2 border-[var(--primary)]"
    } else {
        "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-850/60"
    };

    let mut sidebar_nav = format!(
        r#"<a href="/admin/dashboard" class="flex items-center gap-2.5 px-3 py-2 rounded-md text-xs transition-colors {dash_classes}">
            <svg class="w-4 h-4 shrink-0 text-zinc-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><rect width="7" height="9" x="3" y="3" rx="1"/><rect width="7" height="5" x="14" y="3" rx="1"/><rect width="7" height="9" x="14" y="12" rx="1"/><rect width="7" height="5" x="3" y="16" rx="1"/></svg>
            <span>Dashboard</span>
        </a>"#
    );

    for res in resources {
        if !res.can_view(user) {
            continue;
        }

        let is_active = res.slug() == current_slug;
        let active_classes = if is_active {
            "bg-zinc-800 text-zinc-100 font-medium border-l-2 border-[var(--primary)]"
        } else {
            "text-zinc-400 hover:text-zinc-200 hover:bg-zinc-850/60"
        };

        let icon_svg = match res.slug() {
            "users" => r#"<svg class="w-4 h-4 shrink-0 text-zinc-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87"/><path d="M16 3.13a4 4 0 0 1 0 7.75"/></svg>"#,
            "orders" => r#"<svg class="w-4 h-4 shrink-0 text-zinc-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><circle cx="8" cy="21" r="1"/><circle cx="19" cy="21" r="1"/><path d="M2.05 2.05h2l2.66 12.42a2 2 0 0 0 2 1.58h9.78a2 2 0 0 0 1.95-1.57l1.65-7.43H5.12"/></svg>"#,
            "audit" => r#"<svg class="w-4 h-4 shrink-0 text-zinc-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>"#,
            _ => r#"<svg class="w-4 h-4 shrink-0 text-zinc-400" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><rect width="18" height="18" x="3" y="3" rx="2"/><path d="M9 3v18"/></svg>"#,
        };

        sidebar_nav.push_str(&format!(
            r#"<a href="/admin/{slug}" class="flex items-center gap-2.5 px-3 py-2 rounded-md text-xs transition-colors {active_classes}">
                {icon_svg}
                <span>{name}</span>
            </a>"#,
            slug = res.slug(),
            name = res.plural_name(),
            icon_svg = icon_svg,
            active_classes = active_classes
        ));
    }

    let toast_html = if let Some(msg) = flash_message {
        format!(
            r#"<div id="flash-toast" class="fixed bottom-5 right-5 z-50 flex items-center gap-2 px-3.5 py-2.5 rounded-lg bg-zinc-900 border border-zinc-700 shadow-xl text-xs text-zinc-200 animate-fade-in">
                <svg class="w-4 h-4 text-emerald-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"></path></svg>
                <span>{msg}</span>
                <button type="button" onclick="document.getElementById('flash-toast').remove()" class="ml-2 text-zinc-500 hover:text-zinc-300">&times;</button>
            </div>"#
        )
    } else {
        String::new()
    };

    let brand_name = &theme.brand_name;
    let font_css_url = theme.font_family.css_url();
    let font_family_css = theme.font_family.font_family_css();
    let primary_hex = theme.primary_color.hex();
    let primary_hover_hex = theme.primary_color.hover_hex();
    let primary_light = theme.primary_color.light_bg();

    let brand_logo_html = if let Some(ref custom_logo) = theme.brand_logo {
        custom_logo.clone()
    } else {
        format!(
            r#"<div class="w-7 h-7 rounded-lg bg-zinc-800 border border-zinc-700/80 flex items-center justify-center text-[{primary_hex}]">
                <svg class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="{primary_hex}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <polygon points="12 2 2 7 12 12 22 7 12 2"></polygon>
                    <polyline points="2 17 12 22 22 17"></polyline>
                    <polyline points="2 12 12 17 22 12"></polyline>
                </svg>
            </div>"#
        )
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en" class="dark">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title} · {brand_name}</title>
    <script src="https://cdn.tailwindcss.com"></script>
    <link rel="stylesheet" href="{font_css_url}">
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/geist@1.3.0/dist/fonts/geist-mono/style.css">
    <script>
        tailwind.config = {{
            darkMode: 'class',
            theme: {{
                extend: {{
                    fontFamily: {{
                        sans: [{font_family_css}],
                        mono: ['"Geist Mono"', 'ui-monospace', 'monospace'],
                    }},
                    colors: {{
                        primary: {{
                            500: '{primary_hex}',
                            600: '{primary_hover_hex}',
                        }},
                        zinc: {{
                            950: '#09090b',
                            900: '#121215',
                            850: '#18181c',
                            800: '#27272a',
                            750: '#323238',
                            700: '#3f3f46',
                            500: '#71717a',
                            400: '#a1a1aa',
                            300: '#d4d4d8',
                            100: '#fafafa',
                        }}
                    }}
                }}
            }}
        }}
    </script>
    <style>
        :root {{
            --primary: {primary_hex};
            --primary-hover: {primary_hover_hex};
            --primary-light: {primary_light};
        }}
        body {{ font-family: {font_family_css}; }}
        .loading-shimmer {{ opacity: 0.6; pointer-events: none; transition: opacity 0.15s ease; }}
        dialog::backdrop {{ background: rgba(0, 0, 0, 0.7); backdrop-filter: blur(4px); }}
        @keyframes fadeIn {{ from {{ opacity: 0; transform: translateY(4px); }} to {{ opacity: 1; transform: translateY(0); }} }}
        .animate-fade-in {{ animation: fadeIn 0.15s cubic-bezier(0.16, 1, 0.3, 1) forwards; }}
    </style>
</head>
<body class="bg-zinc-950 text-zinc-100 min-h-screen flex antialiased selection:bg-zinc-800 selection:text-white">
    <!-- Sidebar -->
    <aside class="w-60 bg-zinc-900 border-r border-zinc-800 flex flex-col shrink-0">
        <!-- Brand Header (Clean, zero marketing badges) -->
        <div class="h-14 flex items-center px-4 border-b border-zinc-800">
            <div class="flex items-center gap-2.5">
                {brand_logo_html}
                <span class="text-xs font-semibold tracking-tight text-zinc-100">{brand_name}</span>
            </div>
        </div>

        <!-- Navigation Links -->
        <div class="p-3 flex-1 space-y-1">
            <div class="px-2 pb-1.5 text-[10px] font-semibold tracking-wider text-zinc-500 uppercase font-mono">WORKSPACE</div>
            <nav class="space-y-0.5">
                {sidebar_nav}
            </nav>
        </div>

        <!-- User Profile & Sign Out -->
        <div class="p-3 border-t border-zinc-800 flex items-center justify-between">
            <div class="flex items-center gap-2 min-w-0">
                <div class="w-6 h-6 rounded-full bg-zinc-800 border border-zinc-700 flex items-center justify-center text-[10px] font-mono font-medium text-zinc-300">
                    {user_initials}
                </div>
                <div class="min-w-0">
                    <span class="block text-xs font-medium text-zinc-200 truncate">{user_name}</span>
                    <span class="block text-[10px] text-zinc-400 font-mono truncate">{user_role}</span>
                </div>
            </div>
            <a href="/admin/logout" title="Sign Out" class="p-1 rounded text-zinc-500 hover:text-zinc-300 hover:bg-zinc-800 transition-colors">
                <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.75" d="M17 16l4-4m0 0l-4-4m4 4H7m6 4v1a3 3 0 01-3 3H6a3 3 0 01-3-3V7a3 3 0 013-3h4a3 3 0 013 3v1"></path></svg>
            </a>
        </div>
    </aside>

    <!-- Main Content Area -->
    <main class="flex-1 flex flex-col min-w-0 overflow-y-auto">
        <!-- Topbar (Clean breadcrumbs, zero marketing text) -->
        <header class="h-14 border-b border-zinc-800 bg-zinc-950/80 flex items-center justify-between px-8 backdrop-blur-md sticky top-0 z-10">
            <div class="flex items-center gap-2 text-xs">
                <span class="text-zinc-500">Workspace</span>
                <span class="text-zinc-600">/</span>
                <span class="text-zinc-200 font-medium">{title}</span>
            </div>
            <div class="flex items-center gap-2.5">
                <div id="live-presence-indicator" class="hidden items-center gap-1.5 px-2.5 py-0.5 rounded-full bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 text-[10px] font-mono transition-all">
                    <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
                    <span id="presence-text">Live Sync</span>
                </div>
                <span class="text-[10px] font-mono px-2 py-0.5 rounded border border-zinc-700/80 bg-zinc-850 text-zinc-300">{user_role}</span>
                <span class="text-xs text-zinc-500 font-mono">Ctrl+K</span>
            </div>
        </header>

        <!-- Page Body -->
        <div class="p-8 max-w-7xl w-full mx-auto">
            {content_html}
        </div>
    </main>

    {dialogs_html}
    {toast_html}

    <!-- Clean Client Script for Table Refresh and Native Dialogs -->
    <script>
        let searchTimeout;
        function updateTable(slug, params = {{}}) {{
            const container = document.getElementById('table-wrapper');
            if (container) container.classList.add('loading-shimmer');

            const url = new URL(window.location.origin + '/admin/' + slug + '/table');
            const searchInput = document.getElementById('table-search-input');
            if (searchInput && searchInput.value) {{
                url.searchParams.set('search', searchInput.value);
            }}
            for (const [k, v] of Object.entries(params)) {{
                url.searchParams.set(k, v);
            }}

            fetch(url.toString(), {{ headers: {{ 'X-Requested-With': 'XMLHttpRequest' }} }})
                .then(r => r.text())
                .then(html => {{
                    if (container) {{
                        container.innerHTML = html;
                        container.classList.remove('loading-shimmer');
                        const newSearch = document.getElementById('table-search-input');
                        if (newSearch && params.search_cursor) {{
                            newSearch.focus();
                            newSearch.setSelectionRange(params.search_cursor, params.search_cursor);
                        }}
                    }}
                }})
                .catch(err => {{
                    console.error('Update failed', err);
                    if (container) container.classList.remove('loading-shimmer');
                }});
        }}

        function onSearchInput(slug, e) {{
            clearTimeout(searchTimeout);
            const cursor = e.target.selectionStart;
            searchTimeout = setTimeout(() => {{
                updateTable(slug, {{ search_cursor: cursor }});
            }}, 150);
        }}

        function onSortClick(slug, column, currentSort, currentDesc) {{
            const isSame = currentSort === column;
            const newDesc = isSame ? (currentDesc === 'true' ? 'false' : 'true') : 'false';
            updateTable(slug, {{ sortBy: column, sortDesc: newDesc }});
        }}

        function onFilterChange(slug, name, val) {{
            const url = new URL(window.location.href);
            const paramName = 'filter' + name.charAt(0).toUpperCase() + name.slice(1);
            if (val) {{
                url.searchParams.set(paramName, val);
            }} else {{
                url.searchParams.delete(paramName);
                url.searchParams.delete('filter_' + name);
            }}
            url.searchParams.set('page', '1');
            window.location.href = url.toString();
        }}

        function onPageClick(slug, page) {{
            updateTable(slug, {{ page: page }});
        }}

        // Native HTML5 Dialog Triggers
        function openCreateDialog() {{
            const dlg = document.getElementById('create-dialog');
            if (dlg) dlg.showModal();
        }}

        function openEditDialog(slug, id, valuesJson) {{
            const values = JSON.parse(valuesJson);
            const form = document.getElementById('edit-form');
            if (form) form.action = '/admin/' + slug + '/edit/' + id;

            for (const [key, val] of Object.entries(values)) {{
                let input = document.getElementById('edit-field-' + key);
                if (!input) {{
                    const camelKey = key.replace(/_([a-z])/g, g => g[1].toUpperCase());
                    input = document.getElementById('edit-field-' + camelKey);
                }}
                if (!input) {{
                    const snakeKey = key.replace(/[A-Z]/g, letter => `_${{letter.toLowerCase()}}`);
                    input = document.getElementById('edit-field-' + snakeKey);
                }}
                if (input) {{
                    input.value = val;
                    const previewImg = document.getElementById('img-' + input.id);
                    if (previewImg) {{
                        previewImg.src = val ? val : 'https://placehold.co/128x128/27272a/71717a?text=Preview';
                    }}
                    const picker = document.getElementById('picker-' + input.id);
                    if (picker && typeof val === 'string' && val.startsWith('#')) {{
                        picker.value = val;
                    }}
                }}
            }}

            const dlg = document.getElementById('edit-dialog');
            if (dlg) dlg.showModal();
        }}

        function formatRichText(id, before, after) {{
            const el = document.getElementById(id);
            if (!el) return;
            const start = el.selectionStart || 0;
            const end = el.selectionEnd || 0;
            const text = el.value || '';
            const selected = text.substring(start, end);
            const replacement = before + selected + after;
            el.value = text.substring(0, start) + replacement + text.substring(end);
            el.focus();
            el.selectionStart = start + before.length;
            el.selectionEnd = start + before.length + selected.length;
        }}

        function updateImagePreview(id, url) {{
            const img = document.getElementById('img-' + id);
            if (img) {{
                img.src = (url && url.trim() !== '') ? url : 'https://placehold.co/128x128/27272a/71717a?text=Preview';
            }}
        }}

        function openDeleteDialog(slug, id) {{
            const form = document.getElementById('delete-form');
            if (form) form.action = '/admin/' + slug + '/delete/' + id;

            const dlg = document.getElementById('delete-dialog');
            if (dlg) dlg.showModal();
        }}

        // Bulk & Row Actions JavaScript Helpers
        function toggleSelectAll(master) {{
            const checkboxes = document.querySelectorAll('.row-select-checkbox');
            checkboxes.forEach(cb => {{ cb.checked = master.checked; }});
            updateBulkToolbar();
        }}

        function onRowCheckboxChange() {{
            const checkboxes = document.querySelectorAll('.row-select-checkbox');
            const allChecked = checkboxes.length > 0 && Array.from(checkboxes).every(cb => cb.checked);
            const master = document.getElementById('select-all-checkbox');
            if (master) master.checked = allChecked;
            updateBulkToolbar();
        }}

        function getSelectedRowIds() {{
            const checkboxes = document.querySelectorAll('.row-select-checkbox:checked');
            return Array.from(checkboxes).map(cb => cb.value);
        }}

        function updateBulkToolbar() {{
            const selected = getSelectedRowIds();
            const toolbar = document.getElementById('bulk-actions-toolbar');
            const countSpan = document.getElementById('selected-count');
            if (toolbar) {{
                if (selected.length > 0) {{
                    toolbar.classList.remove('hidden');
                    toolbar.classList.add('flex');
                    if (countSpan) countSpan.textContent = selected.length + ' selected';
                }} else {{
                    toolbar.classList.add('hidden');
                    toolbar.classList.remove('flex');
                }}
            }}
        }}

        function executeRowAction(slug, actionId, id, requiresConfirm, heading, desc) {{
            if (requiresConfirm && !confirm(desc || 'Are you sure you want to execute this action?')) {{
                return;
            }}
            const form = document.createElement('form');
            form.method = 'POST';
            form.action = '/admin/' + slug + '/action/' + actionId + '/' + id;
            document.body.appendChild(form);
            form.submit();
        }}

        function executeBulkAction(slug, actionId, requiresConfirm, heading, desc) {{
            const selected = getSelectedRowIds();
            if (selected.length === 0) {{
                alert('Please select at least one record.');
                return;
            }}
            if (requiresConfirm && !confirm(desc || ('Are you sure you want to execute this action on ' + selected.length + ' selected records?'))) {{
                return;
            }}
            const form = document.createElement('form');
            form.method = 'POST';
            form.action = '/admin/' + slug + '/bulk-action/' + actionId;
            const input = document.createElement('input');
            input.type = 'hidden';
            input.name = 'ids';
            input.value = selected.join(',');
            form.appendChild(input);
            document.body.appendChild(form);
            form.submit();
        }}

        function executeBulkExport(slug, format) {{
            const selected = getSelectedRowIds();
            let url = '/admin/' + slug + '/export?format=' + format;
            if (selected.length > 0) {{
                url += '&ids=' + encodeURIComponent(selected.join(','));
            }}
            window.location.href = url;
        }}

        // Auto dismiss flash toast
        setTimeout(() => {{
            const t = document.getElementById('flash-toast');
            if (t) t.remove();
        }}, 4000);

        // Real-time WebSocket connection
        (function initWebSocket() {{
            try {{
                const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
                const socket = new WebSocket(proto + '//' + window.location.host + '/admin/ws');
                const indicator = document.getElementById('live-presence-indicator');
                const presenceText = document.getElementById('presence-text');

                socket.onopen = function() {{
                    if (indicator) {{
                        indicator.classList.remove('hidden');
                        indicator.classList.add('flex');
                    }}
                }};

                socket.onmessage = function(event) {{
                    try {{
                        const payload = JSON.parse(event.data);
                        const pathParts = window.location.pathname.split('/');
                        const currentSlug = pathParts[2];

                        if (presenceText && payload.event) {{
                            presenceText.textContent = payload.event;
                            setTimeout(() => {{
                                if (presenceText) presenceText.textContent = 'Live Sync';
                            }}, 3000);
                        }}

                        if (currentSlug && (payload.channel === 'admin' || payload.channel === currentSlug)) {{
                            if (typeof updateTable === 'function') {{
                                updateTable(currentSlug);
                            }}
                        }}
                    }} catch(e) {{}}
                }};

                socket.onclose = function() {{
                    if (indicator) {{
                        indicator.classList.add('hidden');
                        indicator.classList.remove('flex');
                    }}
                    setTimeout(initWebSocket, 4000);
                }};
            }} catch(err) {{}}
        }})();
    </script>
</body>
</html>"#,
        title = title,
        sidebar_nav = sidebar_nav,
        content_html = content_html,
        dialogs_html = dialogs_html,
        toast_html = toast_html,
        user_initials = user.initials(),
        user_name = user.name,
        user_role = user.role.as_str()
    )
}

pub fn render_forbidden_page(user: &crate::domain::User, resource_name: &str) -> String {
    format!(
        r#"<div class="py-16 text-center max-w-md mx-auto">
            <div class="w-12 h-12 rounded-2xl bg-rose-500/10 border border-rose-500/20 text-rose-400 mx-auto flex items-center justify-center mb-4">
                <svg class="w-6 h-6" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"></path></svg>
            </div>
            <h2 class="text-base font-semibold text-zinc-100 mb-1">Access Denied (403 Forbidden)</h2>
            <p class="text-xs text-zinc-400 mb-6">Your current role (<span class="font-mono text-zinc-300">{role}</span>) does not have authorization to access <span class="text-zinc-200 font-medium">{resource}</span>.</p>
            <a href="/admin" class="inline-flex items-center gap-2 px-3.5 py-1.5 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-zinc-200 text-xs font-medium border border-zinc-700 transition-colors">
                <span>Return to Workspace</span>
            </a>
        </div>"#,
        role = user.role.as_str(),
        resource = resource_name
    )
}
