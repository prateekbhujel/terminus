//! Fullstack HTML view renderer for Terminus web applications.
//!
//! Provides a Blade/Rails-style View engine that integrates automatically
//! with sessions, one-time flash messages, CSRF tokens, authenticated users,
//! and responsive Tailwind CSS layouts.

use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};
use crate::domain::User;
use crate::helpers::{auth, session};
use std::collections::HashMap;

/// Represents an HTML view ready to be rendered and returned from a web handler.
#[derive(Debug, Clone)]
pub struct View {
    pub title: String,
    pub content: String,
    pub status: StatusCode,
    pub layout: bool,
    pub flash_bag: HashMap<String, String>,
    pub user: Option<User>,
    pub csrf_token: Option<String>,
}

impl View {
    /// Creates a new View rendered within the master application layout.
    pub fn new(title: impl Into<String>, content: impl Into<String>) -> Self {
        let sess = session();
        let flash = sess.all_flashes();
        sess.sweep_flashes();
        let csrf = sess.csrf_token();
        let u = auth();

        Self {
            title: title.into(),
            content: content.into(),
            status: StatusCode::OK,
            layout: true,
            flash_bag: flash,
            user: u,
            csrf_token: Some(csrf),
        }
    }

    /// Creates a View that renders raw HTML without the layout wrapper.
    pub fn raw(html: impl Into<String>) -> Self {
        let mut v = Self::new("", html);
        v.layout = false;
        v
    }

    /// Sets the view title rendered in the page header and document title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Sets the HTTP status code for this view response.
    pub fn status(mut self, status: StatusCode) -> Self {
        self.status = status;
        self
    }

    /// Disables the master layout wrapper.
    pub fn without_layout(mut self) -> Self {
        self.layout = false;
        self
    }

    /// Sets an explicit authenticated user for this view.
    pub fn with_user(mut self, user: User) -> Self {
        self.user = Some(user);
        self
    }

    /// Injects an additional flash message into this view.
    pub fn with_flash(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.flash_bag.insert(key.into(), val.into());
        self
    }

    /// Renders the complete HTML string.
    pub fn render(&self) -> String {
        if !self.layout {
            return self.content.clone();
        }

        let mut flash_html = String::new();
        for (key, msg) in &self.flash_bag {
            let (bg_border, text_color, badge) = match key.as_str() {
                "success" | "ok" | "status" => (
                    "bg-emerald-950/60 border-emerald-800/80 text-emerald-200",
                    "text-emerald-400",
                    "SUCCESS",
                ),
                "error" | "danger" | "failed" => (
                    "bg-rose-950/60 border-rose-800/80 text-rose-200",
                    "text-rose-400",
                    "ERROR",
                ),
                "warning" => (
                    "bg-amber-950/60 border-amber-800/80 text-amber-200",
                    "text-amber-400",
                    "WARNING",
                ),
                _ => (
                    "bg-sky-950/60 border-sky-800/80 text-sky-200",
                    "text-sky-400",
                    "INFO",
                ),
            };
            flash_html.push_str(&format!(
                r#"<div class="mb-6 p-4 rounded-xl border {} flex items-center justify-between shadow-lg">
                    <div class="flex items-center gap-3">
                        <span class="px-2 py-0.5 rounded text-[10px] font-bold tracking-wider uppercase bg-black/40 {}">{}</span>
                        <span class="text-sm font-medium">{}</span>
                    </div>
                </div>"#,
                bg_border, text_color, badge, msg
            ));
        }

        let user_nav = if let Some(u) = &self.user {
            format!(
                r#"<div class="flex items-center gap-3">
                    <span class="text-xs text-zinc-400">{}</span>
                    <a href="/admin" class="px-3 py-1.5 rounded-lg bg-zinc-800 hover:bg-zinc-700 text-xs text-zinc-200 font-medium transition-colors">Admin Panel</a>
                </div>"#,
                u.email
            )
        } else {
            r#"<div class="flex items-center gap-2">
                <a href="/admin/login" class="px-3.5 py-1.5 rounded-lg bg-indigo-600 hover:bg-indigo-500 text-xs text-white font-medium transition-colors shadow-sm">Sign In</a>
            </div>"#.to_string()
        };

        let csrf_meta = self
            .csrf_token
            .as_ref()
            .map(|t| format!(r#"<meta name="csrf-token" content="{}">"#, t))
            .unwrap_or_default();

        format!(
            r#"<!DOCTYPE html>
<html lang="en" class="h-full bg-zinc-950 text-zinc-100">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    {csrf_meta}
    <title>{title} - Terminus</title>
    <script src="https://cdn.tailwindcss.com"></script>
    <style>
        :root {{ color-scheme: dark; }}
    </style>
</head>
<body class="min-h-full flex flex-col font-sans antialiased bg-zinc-950 text-zinc-100">
    <header class="border-b border-zinc-800/80 bg-zinc-900/60 backdrop-blur sticky top-0 z-40">
        <div class="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8 h-16 flex items-center justify-between">
            <div class="flex items-center gap-6">
                <a href="/" class="flex items-center gap-2.5 font-bold tracking-tight text-white hover:text-indigo-400 transition-colors">
                    <div class="w-8 h-8 rounded-lg bg-indigo-600 flex items-center justify-center text-white font-black text-sm shadow-md shadow-indigo-600/30">T</div>
                    <span class="text-base font-semibold tracking-wide">Terminus</span>
                </a>
                <nav class="hidden md:flex items-center gap-1 text-sm font-medium">
                    <a href="/" class="px-3 py-1.5 rounded-md text-zinc-300 hover:text-white hover:bg-zinc-800/60 transition-colors">Home</a>
                    <a href="/admin" class="px-3 py-1.5 rounded-md text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/40 transition-colors">Admin Dashboard</a>
                </nav>
            </div>
            {user_nav}
        </div>
    </header>

    <main class="flex-1 max-w-7xl w-full mx-auto px-4 sm:px-6 lg:px-8 py-8">
        {flash_html}
        {content}
    </main>

    <footer class="border-t border-zinc-800/60 bg-zinc-950/80 py-6 text-center text-xs text-zinc-500">
        <div class="max-w-7xl mx-auto px-4">
            Powered by <a href="https://github.com/prateekbhujel/terminus" class="text-zinc-400 hover:text-zinc-200 transition-colors">Terminus Framework</a> - Universal Fullstack Application Engine
        </div>
    </footer>
</body>
</html>"#,
            csrf_meta = csrf_meta,
            title = self.title,
            user_nav = user_nav,
            flash_html = flash_html,
            content = self.content
        )
    }
}

impl IntoResponse for View {
    fn into_response(self) -> Response {
        (self.status, Html(self.render())).into_response()
    }
}

/// Helper constructor to render a web page wrapped in the default layout.
pub fn view(title: impl Into<String>, content: impl Into<String>) -> View {
    View::new(title, content)
}

/// Helper constructor to render a raw HTML string.
pub fn raw_html(content: impl Into<String>) -> View {
    View::raw(content)
}
