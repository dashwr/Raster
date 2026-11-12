//! Metadados de versão embutidos no binário: commit, se a árvore estava suja e data do build.
fn main() {
    let git = |args: &[&str]| std::process::Command::new("git").args(args).output().ok()
        .filter(|o| o.status.success()).map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    let mut rev = std::env::var("RASTER_REV").unwrap_or_else(|_| git(&["rev-parse", "--short=7", "HEAD"]));
    if rev.is_empty() {
        rev = "unknown".into();
    }
    let date = std::process::Command::new("date").args(["-u", "+%Y-%m-%d"]).output().ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
    println!("cargo:rustc-env=RASTER_REV={rev}");
    println!("cargo:rustc-env=RASTER_BUILD_DATE={date}");
    println!("cargo:rerun-if-env-changed=RASTER_REV");
    println!("cargo:rerun-if-changed=.git/HEAD");
}
