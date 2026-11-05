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

            <form method="POST" action="/admin/login" class="space-y-4">
                <input type="hidden" name="_csrf_token" value="{csrf_token}">
                <div>
                    <label class="block text-xs font-medium text-zinc-300 mb-1.5" for="email">Email address</label>
                    <input 
                        type="email" 
                        id="email" 
                        name="email" 
                        required 
                        class="w-full px-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-sm text-zinc-100 placeholder-zinc-500 focus:outline-none focus:ring-1 focus:ring-zinc-600 focus:border-zinc-600 transition-all font-sans"
                        placeholder="admin@example.com"
                    />
                </div>

                <div>
                    <div class="flex items-center justify-between mb-1.5">
                        <label class="text-xs font-medium text-zinc-300" for="password">Password</label>
                    </div>
                    <input 
                        type="password" 
                        id="password" 
                        name="password" 
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


        </div>
    </div>

</body>
</html>"#,
        error_banner = error_banner,
        csrf_token = csrf_token
    )
}

/// Password hashing using Argon2id, with verification support for existing PBKDF2 hashes.
pub struct Hash;

impl Hash {
    /// Hash a password with Argon2id and an operating-system-generated salt.
    pub fn make(password: &str) -> String {
        Self::try_make(password).expect("Unable to hash password with operating-system entropy")
    }

    /// Fallible password hashing for applications that need to report entropy or hashing errors.
    pub fn try_make(password: &str) -> Result<String, argon2::password_hash::Error> {
        use argon2::password_hash::PasswordHasher;
        argon2::Argon2::default().hash_password(password.as_bytes()).map(|hash| hash.to_string())
    }

    /// Create a legacy PBKDF2 hash for compatibility. Use `make` for new passwords.
    pub fn make_with_salt(password: &str, salt: &str, iterations: u32) -> String {
        let derived = pbkdf2_sha256(password.as_bytes(), salt.as_bytes(), iterations);
        let mut hex = String::with_capacity(64);
        for b in derived {
            hex.push_str(&format!("{:02x}", b));
        }
        format!("$pbkdf2-sha256$i={}${}${}", iterations, salt, hex)
    }

    /// Verify an Argon2id or legacy PBKDF2 hash. Plaintext values are rejected.
    pub fn check(password: &str, hash: &str) -> bool {
        if hash.starts_with("$argon2id$") {
            use argon2::password_hash::{phc::PasswordHash, PasswordVerifier};
            return PasswordHash::new(hash).ok().is_some_and(|parsed| {
                argon2::Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
            });
        }
        let parts: Vec<&str> = hash.split('$').collect();
        if parts.len() != 5 || !parts[0].is_empty() || parts[1] != "pbkdf2-sha256"
            || parts[3].is_empty() || parts[3].len() > 1024
            || parts[4].len() != 64 || !parts[4].bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return false;
        }
        let Some(iterations) = parts[2].strip_prefix("i=")
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|iterations| (1..=1_000_000).contains(iterations)) else {
                return false;
            };

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
