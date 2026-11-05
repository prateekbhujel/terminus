use crate::domain::audit::AuditLog;
use crate::domain::user::User;

pub fn render_dashboard(
    user: &User,
    total_orders: usize,
    gross_revenue: &str,
    total_users: usize,
    recent_audits: &[AuditLog],
) -> String {
    let mut audit_rows = String::new();

    if recent_audits.is_empty() {
        audit_rows.push_str(
            r#"<tr><td colspan="4" class="px-5 py-8 text-center text-zinc-500 text-xs font-mono">No recent activity recorded yet.</td></tr>"#
        );
    } else {
        for log in recent_audits.iter().take(5) {
            let (badge_bg, badge_text, badge_border) = match log.action.as_str() {
                "CREATE" => ("bg-emerald-500/10", "text-emerald-400", "border-emerald-500/20"),
                "UPDATE" => ("bg-blue-500/10", "text-blue-400", "border-blue-500/20"),
                "DELETE" => ("bg-rose-500/10", "text-rose-400", "border-rose-500/20"),
                _ => ("bg-zinc-800", "text-zinc-400", "border-zinc-700"),
            };

            audit_rows.push_str(&format!(
                r#"<tr class="hover:bg-zinc-850/50 transition-colors border-b border-zinc-800/60 last:border-b-0">
                    <td class="px-5 py-3 whitespace-nowrap text-xs text-zinc-400 font-mono">{time}</td>
                    <td class="px-5 py-3 whitespace-nowrap text-xs text-zinc-200 font-medium">{user}</td>
                    <td class="px-5 py-3 whitespace-nowrap text-xs">
                        <span class="inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-medium border {badge_bg} {badge_text} {badge_border}">{action}</span>
                    </td>
                    <td class="px-5 py-3 text-xs text-zinc-400 truncate max-w-xs">{details}</td>
                </tr>"#,
                time = log.timestamp,
                user = log.user_name,
                action = log.action,
                details = log.details,
                badge_bg = badge_bg,
                badge_text = badge_text,
                badge_border = badge_border
            ));
        }
    }

    format!(
        r#"<div class="space-y-8 max-w-7xl mx-auto">
            <!-- Welcome Header -->
            <div class="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
                <div>
                    <h1 class="text-xl font-semibold tracking-tight text-zinc-100">Welcome back, {user_name}</h1>
                    <p class="text-xs text-zinc-400 mt-1">Operational workspace running on Axum 0.7 & SQLite persistence.</p>
                </div>
                <div class="flex items-center gap-2">
                    <span class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full bg-emerald-500/10 border border-emerald-500/20 text-emerald-400 text-[11px] font-mono">
                        <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
                        <span>Engine Online</span>
                    </span>
                    <span class="text-xs font-mono text-zinc-500 px-2 py-1 bg-zinc-900 border border-zinc-800 rounded-lg">v0.1 Alpha</span>
                </div>
            </div>

            <!-- KPI Metric Cards Grid -->
            <div class="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
                <!-- Card 1: Gross Revenue -->
                <div class="bg-zinc-900 border border-zinc-800 rounded-xl p-5 shadow-sm">
                    <div class="flex items-center justify-between text-zinc-400 text-xs mb-3">
                        <span class="font-medium text-zinc-400">Total Revenue</span>
                        <div class="w-7 h-7 rounded-lg bg-emerald-500/10 text-emerald-400 flex items-center justify-center">
                            <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z"></path></svg>
                        </div>
                    </div>
                    <div class="text-2xl font-semibold text-zinc-100 font-mono tracking-tight">{gross_revenue}</div>
                    <div class="mt-2 flex items-center gap-1.5 text-[11px] text-emerald-400">
                        <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 7h8m0 0v8m0-8l-8 8-4-4-6 6"></path></svg>
                        <span>+14.8% vs last month</span>
                    </div>
                </div>

                <!-- Card 2: Total Orders -->
                <div class="bg-zinc-900 border border-zinc-800 rounded-xl p-5 shadow-sm">
                    <div class="flex items-center justify-between text-zinc-400 text-xs mb-3">
                        <span class="font-medium text-zinc-400">Orders Processed</span>
                        <div class="w-7 h-7 rounded-lg bg-blue-500/10 text-blue-400 flex items-center justify-center">
                            <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M16 11V7a4 4 0 00-8 0v4M5 9h14l1 12H4L5 9z"></path></svg>
                        </div>
                    </div>
                    <div class="text-2xl font-semibold text-zinc-100 font-mono tracking-tight">{total_orders}</div>
                    <div class="mt-2 flex items-center gap-1.5 text-[11px] text-zinc-400 font-mono">
                        <span>Persistent SQLite storage</span>
                    </div>
                </div>

                <!-- Card 3: Active Members -->
                <div class="bg-zinc-900 border border-zinc-800 rounded-xl p-5 shadow-sm">
                    <div class="flex items-center justify-between text-zinc-400 text-xs mb-3">
                        <span class="font-medium text-zinc-400">Team Accounts</span>
                        <div class="w-7 h-7 rounded-lg bg-indigo-500/10 text-indigo-400 flex items-center justify-center">
                            <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 20h5v-2a3 3 0 00-5.356-1.857M17 20H7m10 0v-2c0-.656-.126-1.283-.356-1.857M7 20H2v-2a3 3 0 015.356-1.857M7 20v-2c0-.656.126-1.283.356-1.857m0 0a5.002 5.002 0 019.288 0M15 7a3 3 0 11-6 0 3 3 0 016 0zm6 3a2 2 0 11-4 0 2 2 0 014 0zM7 10a2 2 0 11-4 0 2 2 0 014 0z"></path></svg>
                        </div>
                    </div>
                    <div class="text-2xl font-semibold text-zinc-100 font-mono tracking-tight">{total_users}</div>
                    <div class="mt-2 flex items-center gap-1.5 text-[11px] text-zinc-400 font-mono">
                        <span>RBAC Policy Protected</span>
                    </div>
                </div>

                <!-- Card 4: Security Events -->
                <div class="bg-zinc-900 border border-zinc-800 rounded-xl p-5 shadow-sm">
                    <div class="flex items-center justify-between text-zinc-400 text-xs mb-3">
                        <span class="font-medium text-zinc-400">Audit Events</span>
                        <div class="w-7 h-7 rounded-lg bg-amber-500/10 text-amber-400 flex items-center justify-center">
                            <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z"></path></svg>
                        </div>
                    </div>
                    <div class="text-2xl font-semibold text-zinc-100 font-mono tracking-tight">{recent_count}</div>
                    <div class="mt-2 flex items-center gap-1.5 text-[11px] text-zinc-400">
                        <span>Immutable audit logs</span>
                    </div>
                </div>
            </div>

            <!-- Lower Grid: Recent Activity Feed & Quick Actions -->
            <div class="grid grid-cols-1 lg:grid-cols-3 gap-6">
                <!-- Recent Activity Table (2 columns wide) -->
                <div class="lg:col-span-2 bg-zinc-900 border border-zinc-800 rounded-xl overflow-hidden shadow-sm">
                    <div class="px-5 py-4 border-b border-zinc-800 flex items-center justify-between">
                        <div>
                            <h3 class="text-xs font-semibold text-zinc-100 uppercase tracking-wider font-mono">Recent Activity</h3>
                            <p class="text-[11px] text-zinc-400 mt-0.5">Live mutation log from persistent SQLite audit trail</p>
                        </div>
                        <a href="/admin/audit" class="text-xs text-zinc-400 hover:text-zinc-100 flex items-center gap-1 transition-colors">
                            <span>View All</span>
                            <svg class="w-3 h-3" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7"></path></svg>
                        </a>
                    </div>
                    <div class="overflow-x-auto">
                        <table class="min-w-full divide-y divide-zinc-800">
                            <thead class="bg-zinc-850/60">
                                <tr>
                                    <th class="px-5 py-2.5 text-left text-[11px] font-medium text-zinc-500 uppercase tracking-wider">Timestamp</th>
                                    <th class="px-5 py-2.5 text-left text-[11px] font-medium text-zinc-500 uppercase tracking-wider">Operator</th>
                                    <th class="px-5 py-2.5 text-left text-[11px] font-medium text-zinc-500 uppercase tracking-wider">Action</th>
                                    <th class="px-5 py-2.5 text-left text-[11px] font-medium text-zinc-500 uppercase tracking-wider">Details</th>
                                </tr>
                            </thead>
                            <tbody class="divide-y divide-zinc-800/80">
                                {audit_rows}
                            </tbody>
                        </table>
                    </div>
                </div>

                <!-- Quick Operations & System Specs -->
                <div class="space-y-4">
                    <div class="bg-zinc-900 border border-zinc-800 rounded-xl p-5 shadow-sm">
                        <h3 class="text-xs font-semibold text-zinc-100 uppercase tracking-wider font-mono mb-3">Quick Navigation</h3>
                        <div class="space-y-2">
                            <a href="/admin/orders" class="flex items-center justify-between p-2.5 rounded-lg bg-zinc-950/60 hover:bg-zinc-800/60 border border-zinc-800 transition-colors group">
                                <div class="flex items-center gap-2.5">
                                    <div class="w-7 h-7 rounded bg-blue-500/10 text-blue-400 flex items-center justify-center">
                                        <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><circle cx="8" cy="21" r="1"/><circle cx="19" cy="21" r="1"/><path d="M2.05 2.05h2l2.66 12.42a2 2 0 0 0 2 1.58h9.78a2 2 0 0 0 1.95-1.57l1.65-7.43H5.12"/></svg>
                                    </div>
                                    <span class="text-xs text-zinc-200 group-hover:text-white font-medium">Manage Orders</span>
                                </div>
                                <span class="text-[10px] text-zinc-500 font-mono">Drawer / CRUD &rarr;</span>
                            </a>

                            <a href="/admin/users" class="flex items-center justify-between p-2.5 rounded-lg bg-zinc-950/60 hover:bg-zinc-800/60 border border-zinc-800 transition-colors group">
                                <div class="flex items-center gap-2.5">
                                    <div class="w-7 h-7 rounded bg-indigo-500/10 text-indigo-400 flex items-center justify-center">
                                        <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/></svg>
                                    </div>
                                    <span class="text-xs text-zinc-200 group-hover:text-white font-medium">Manage Users</span>
                                </div>
                                <span class="text-[10px] text-zinc-500 font-mono">Filters / RBAC &rarr;</span>
                            </a>

                            <a href="/admin/audit" class="flex items-center justify-between p-2.5 rounded-lg bg-zinc-950/60 hover:bg-zinc-800/60 border border-zinc-800 transition-colors group">
                                <div class="flex items-center gap-2.5">
                                    <div class="w-7 h-7 rounded bg-amber-500/10 text-amber-400 flex items-center justify-center">
                                        <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/></svg>
                                    </div>
                                    <span class="text-xs text-zinc-200 group-hover:text-white font-medium">Security Audit Trail</span>
                                </div>
                                <span class="text-[10px] text-zinc-500 font-mono">Audit Logs &rarr;</span>
                            </a>
                        </div>
                    </div>

                    <!-- Architecture Overview Card -->
                    <div class="bg-zinc-900 border border-zinc-800 rounded-xl p-5 shadow-sm">
                        <h3 class="text-xs font-semibold text-zinc-100 uppercase tracking-wider font-mono mb-2.5">System Architecture</h3>
                        <div class="space-y-2 text-xs text-zinc-400">
                            <div class="flex justify-between py-1 border-b border-zinc-800/80">
                                <span class="text-zinc-500">HTTP Framework</span>
                                <span class="text-zinc-300 font-mono text-[11px]">Axum 0.7</span>
                            </div>
                            <div class="flex justify-between py-1 border-b border-zinc-800/80">
                                <span class="text-zinc-500">Database Driver</span>
                                <span class="text-zinc-300 font-mono text-[11px]">rusqlite (bundled)</span>
                            </div>
                            <div class="flex justify-between py-1 border-b border-zinc-800/80">
                                <span class="text-zinc-500">Access Control</span>
                                <span class="text-zinc-300 font-mono text-[11px]">Type-Safe RBAC</span>
                            </div>
                            <div class="flex justify-between py-1">
                                <span class="text-zinc-500">Frontend Stack</span>
                                <span class="text-zinc-300 font-mono text-[11px]">Server HTML5 / Zero JS build</span>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </div>"#,
        user_name = user.name,
        gross_revenue = gross_revenue,
        total_orders = total_orders,
        total_users = total_users,
        recent_count = recent_audits.len(),
        audit_rows = audit_rows
    )
}
