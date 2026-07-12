// Only needed for the server-build pgAdmin 4 pre-build check.
#[cfg(feature = "server-build")]
use std::path::Path;

fn main() {
    // CR-22 follow-up: pre-build check for pgAdmin 4 in the bundled PostgreSQL
    // resources. The EnterpriseDB binaries zip includes pgAdmin 4 (~100+ MB,
    // thousands of files with long paths). NSIS cannot extract these paths
    // (>260 chars) and the installer fails with "error writing to file".
    //
    // This check fails the build EARLY with a clear message instead of
    // letting the user discover the failure mid-install.
    //
    // Only runs for the server-build feature (which bundles PostgreSQL).
    #[cfg(feature = "server-build")]
    {
        let pg_admin_path = Path::new("resources/pgsql/pgAdmin 4");
        if pg_admin_path.exists() {
            panic!(
                "\n\
                 ───────────────────────────────────────────────────────────────────\n\
                 BUILD ABORTED: pgAdmin 4 found in bundled PostgreSQL resources.\n\
                 ───────────────────────────────────────────────────────────────────\n\
                 \n\
                 The folder '{}' exists. The EnterpriseDB binaries zip includes\n\
                 pgAdmin 4, which HMS does NOT use. It adds ~100+ MB and thousands\n\
                 of files with paths exceeding NSIS's 260-character limit, causing\n\
                 the installer to fail with 'error writing to file'.\n\
                 \n\
                 FIX: Delete these folders before building:\n\
                 \n\
                   resources/pgsql/pgAdmin 4/   (REQUIRED — causes installer failure)\n\
                   resources/pgsql/docs/        (optional — saves ~20 MB)\n\
                   resources/pgsql/include/     (optional — saves ~5 MB)\n\
                   resources/pgsql/symbols/     (optional — saves ~10 MB)\n\
                 \n\
                 After cleanup, resources/pgsql/ should contain only:\n\
                   bin/   (pg_ctl, initdb, postgres, pg_isready, psql + DLLs)\n\
                   lib/   (shared libraries)\n\
                   share/ (locale, timezones, error messages, extensions)\n\
                 \n\
                 See src-tauri/SETUP_POSTGRES_BINARIES.md §2a for details.\n\
                 \n\
                 ───────────────────────────────────────────────────────────────────\n",
                pg_admin_path.display()
            );
        }
    }

    tauri_build::build()
}
