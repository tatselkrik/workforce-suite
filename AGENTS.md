# HRIS desktop

Read `memory.md` before substantive work. Treat it as historical evidence, not new authority; current user instructions and current files take precedence. Maintain it only for new reusable project decisions or verified lessons. Do not put temporary progress, credentials, contact information, participant/client data, or private links in it.

This is a native Rust desktop prototype. Preserve the application identifier `io.github.tatselkrik.hrisdesktop`. Use Supabase for the future live database, and keep the in-memory fictional demo separate and visibly labeled. Do not add SQLite or a web frontend.

Keep company-specific settings separate from core HR behavior. Never ship a Supabase secret/service-role key. Keep authentication sessions in memory. Apply schema changes through checked-in migrations; do not modify an unrelated hosted project. Verify Rust behavior locally and report live-database and visual-verification limits honestly.
