pub fn render_login_page(error_msg: Option<&str>) -> String {
    let csrf_token = crate::helpers::csrf_token();
    let error_banner = if let Some(err) = error_msg {
        format!(
            r#"<div class="mb-5 p-3 rounded-md bg-rose-500/10 border border-rose-500/20 text-xs text-rose-400 flex items-center gap-2">
                <svg class="w-4 h-4 shrink-0 text-rose-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4m0 4h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg>
                <span>{err}</span>
            </div>"#
        )
    } else {
        String::new()
    };

    format!(
        r#"<!DOCTYPE html>
<html lang="en" class="dark">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Sign In · Terminus</title>
    <script src="https://cdn.tailwindcss.com"></script>
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/geist@1.3.0/dist/fonts/geist-sans/style.css">
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/npm/geist@1.3.0/dist/fonts/geist-mono/style.css">
    <script>
        tailwind.config = {{
            darkMode: 'class',
            theme: {{
                extend: {{
                    fontFamily: {{
                        sans: ['"Geist Sans"', 'system-ui', 'sans-serif'],
                        mono: ['"Geist Mono"', 'monospace'],
                    }},
                    colors: {{
                        zinc: {{
                            950: '#09090b',
                            900: '#121215',
                            850: '#1a1a1f',
                            800: '#27272a',
                            700: '#3f3f46',
                            400: '#a1a1aa',
                            300: '#d4d4d8',
                            100: '#f4f4f5',
                        }}
                    }}
                }}
            }}
        }}
    </script>
    <style>
        body {{ font-family: 'Geist Sans', system-ui, sans-serif; }}
    </style>
</head>
<body class="bg-zinc-950 text-zinc-100 min-h-screen flex items-center justify-center p-4 antialiased selection:bg-zinc-800 selection:text-white">
    <div class="w-full max-w-sm">
        <!-- Brand Header -->
        <div class="flex flex-col items-center mb-8">
            <div class="w-10 h-10 rounded-xl bg-zinc-900 border border-zinc-800 flex items-center justify-center mb-4 shadow-sm">
                <!-- Geometric Minimalist Monogram -->
                <svg class="w-5 h-5 text-zinc-100" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <polygon points="12 2 2 7 12 12 22 7 12 2"></polygon>
                    <polyline points="2 17 12 22 22 17"></polyline>
                    <polyline points="2 12 12 17 22 12"></polyline>
                </svg>
            </div>
            <h1 class="text-xl font-medium tracking-tight text-zinc-100">Sign in to Terminus</h1>
            <p class="text-xs text-zinc-500 mt-1">Sign in to your account to continue</p>
        </div>

        <!-- Auth Card -->
        <div class="bg-zinc-900/80 border border-zinc-800 rounded-xl p-6 shadow-2xl backdrop-blur-md">
            {error_banner}

            <!-- OAuth Options -->
            <div class="mb-5">
                <a href="/admin/oauth/github" class="w-full py-2 px-3 rounded-lg bg-zinc-800/80 hover:bg-zinc-800 border border-zinc-700/80 text-zinc-200 font-medium text-xs transition-colors flex items-center justify-center gap-2 shadow-sm">
                    <svg class="w-4 h-4 text-zinc-200" viewBox="0 0 24 24" fill="currentColor">
                        <path fill-rule="evenodd" clip-rule="evenodd" d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.53 1.032 1.53 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0112 6.844c.85.004 1.705.115 2.504.337 1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.019 10.019 0 0022 12.017C22 6.484 17.522 2 12 2z"/>
                    </svg>
                    <span>Sign in with GitHub</span>
                </a>
            </div>

            <div class="relative my-4 flex items-center justify-center">
                <div class="border-t border-zinc-800 w-full"></div>
                <span class="bg-zinc-900 px-2 text-[10px] uppercase tracking-wider text-zinc-500 font-mono absolute">or email</span>
            </div>

            <form method="POST" action="/admin/login" class="space-y-4">
                <input type="hidden" name="_csrf_token" value="{csrf_token}">
                <div>
                    <label class="block text-xs font-medium text-zinc-300 mb-1.5" for="email">Email address</label>
                    <input 
                        type="email" 
                        id="email" 
                        name="email" 
                        value="pratik@terminus.dev"
                        required 
                        class="w-full px-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:ring-1 focus:ring-zinc-600 focus:border-zinc-600 transition-all font-sans"
                        placeholder="admin@example.com"
                    />
                </div>

                <div>
                    <div class="flex items-center justify-between mb-1.5">
                        <label class="text-xs font-medium text-zinc-300" for="password">Password</label>
                        <span class="text-[11px] text-zinc-500 font-mono">admin123</span>
                    </div>
                    <input 
                        type="password" 
                        id="password" 
                        name="password" 
                        value="admin123"
                        required 
                        class="w-full px-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:ring-1 focus:ring-zinc-600 focus:border-zinc-600 transition-all font-sans"
                    />
                </div>

                <div class="pt-2">
                    <button 
                        type="submit" 
                        class="w-full py-2 px-4 rounded-lg bg-zinc-100 hover:bg-white text-zinc-900 font-medium text-sm transition-all duration-150 shadow-sm flex items-center justify-center gap-2">
                        <span>Continue</span>
                        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M14 5l7 7m0 0l-7 7m7-7H3"></path></svg>
                    </button>
                </div>
            </form>

            <!-- Quick Demo Role Switcher -->
            <div class="mt-6 pt-5 border-t border-zinc-800">
                <div class="text-[11px] font-medium text-zinc-400 mb-2">Switch Demo Account:</div>
                <div class="grid grid-cols-2 gap-2 text-left">
                    <button type="button" onclick="setDemo('pratik@terminus.dev')" class="p-2 rounded-lg bg-zinc-950/60 hover:bg-zinc-850 border border-zinc-800 text-left transition-colors">
                        <div class="text-xs font-medium text-zinc-200">Pratik</div>
                        <div class="text-[10px] text-indigo-400 font-mono">Superadmin</div>
                    </button>
                    <button type="button" onclick="setDemo('dharma.shrestha@terminus.dev')" class="p-2 rounded-lg bg-zinc-950/60 hover:bg-zinc-850 border border-zinc-800 text-left transition-colors">
                        <div class="text-xs font-medium text-zinc-200">Dharma Raj</div>
                        <div class="text-[10px] text-blue-400 font-mono">Admin</div>
                    </button>
                    <button type="button" onclick="setDemo('elena@terminus.dev')" class="p-2 rounded-lg bg-zinc-950/60 hover:bg-zinc-850 border border-zinc-800 text-left transition-colors">
                        <div class="text-xs font-medium text-zinc-200">Elena Rostova</div>
                        <div class="text-[10px] text-amber-400 font-mono">Editor</div>
                    </button>
                    <button type="button" onclick="setDemo('ranjan.gumanju@terminus.dev')" class="p-2 rounded-lg bg-zinc-950/60 hover:bg-zinc-850 border border-zinc-800 text-left transition-colors">
                        <div class="text-xs font-medium text-zinc-200">Ranjan Gumanju</div>
                        <div class="text-[10px] text-zinc-400 font-mono">Member (Read-only)</div>
                    </button>
                </div>
            </div>
        </div>
    </div>

    <script>
        function setDemo(email) {{
            document.getElementById('email').value = email;
            document.getElementById('password').value = 'admin123';
        }}
    </script>
</body>
</html>"#,
        error_banner = error_banner,
        csrf_token = csrf_token
    )
}

/// Pure-Rust Password Hashing (PBKDF2-HMAC-SHA256 conforming to RFC 2898 / NIST SP 800-132)
pub struct Hash;

impl Hash {
    /// Hash a plaintext password with a random salt and 10,000 iterations
    pub fn make(password: &str) -> String {
        let salt = crate::session::secure_random_hex(16);
        Self::make_with_salt(password, &salt, 10_000)
    }

    /// Hash password with specified salt and iteration count
    pub fn make_with_salt(password: &str, salt: &str, iterations: u32) -> String {
        let derived = pbkdf2_sha256(password.as_bytes(), salt.as_bytes(), iterations);
        let mut hex = String::with_capacity(64);
        for b in derived {
            hex.push_str(&format!("{:02x}", b));
        }
        format!("$pbkdf2-sha256$i={}${}${}", iterations, salt, hex)
    }

    /// Verify a plaintext password against a stored modular hash string or fallback
    pub fn check(password: &str, hash: &str) -> bool {
        let parts: Vec<&str> = hash.split('$').collect();
        // Expected format: ["", "pbkdf2-sha256", "i=10000", "salt", "hex"]
        if parts.len() != 5 || parts[1] != "pbkdf2-sha256" {
            // Constant-time check for demo plaintext passwords
            return crate::token::constant_time_eq_str(password, hash);
        }

        let iterations: u32 = parts[2]
            .strip_prefix("i=")
            .and_then(|s| s.parse().ok())
            .unwrap_or(10_000);
        let salt = parts[3];
        let expected_hex = parts[4];

        let derived = pbkdf2_sha256(password.as_bytes(), salt.as_bytes(), iterations);
        let mut computed_hex = String::with_capacity(64);
        for b in derived {
            computed_hex.push_str(&format!("{:02x}", b));
        }

        crate::token::constant_time_eq_str(&computed_hex, expected_hex)
    }
}

/// PBKDF2 with HMAC-SHA256 key derivation (32-byte key)
fn pbkdf2_sha256(password: &[u8], salt: &[u8], iterations: u32) -> [u8; 32] {
    let mut salt_with_int = Vec::with_capacity(salt.len() + 4);
    salt_with_int.extend_from_slice(salt);
    salt_with_int.extend_from_slice(&1u32.to_be_bytes());

    let mut u = crate::token::hmac_sha256(password, &salt_with_int);
    let mut t = u;

    for _ in 1..iterations {
        u = crate::token::hmac_sha256(password, &u);
        for i in 0..32 {
            t[i] ^= u[i];
        }
    }
    t
}
