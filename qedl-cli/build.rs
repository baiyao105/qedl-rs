use chrono::Local;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::process::Command;

fn run_command(cmd: &str, args: &[&str]) -> Option<String> {
    Command::new(cmd).args(args).output().ok().and_then(|output| {
        if output.status.success() {
            String::from_utf8(output.stdout).ok().map(|s| s.trim().to_string())
        } else {
            None
        }
    })
}

fn write_const(file: &mut File, name: &str, value: &str) {
    writeln!(file, "pub const {}: &str = {:?};", name, value).unwrap();
}

fn main() {
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("version.rs");

    let mut file = File::create(dest_path).unwrap();

    let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string());

    let git_sha = run_command("git", &["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_string());

    let git_branch =
        run_command("git", &["rev-parse", "--abbrev-ref", "HEAD"]).unwrap_or_else(|| "unknown".to_string());

    let build_date = Local::now().format("%Y/%m/%d").to_string();

    let full_version = format!("Version: {} - {} ({}) ({})", version, git_sha, git_branch, build_date);

    write_const(&mut file, "VERSION", &version);
    write_const(&mut file, "GIT_SHA", &git_sha);
    write_const(&mut file, "GIT_BRANCH", &git_branch);
    write_const(&mut file, "BUILD_DATE", &build_date);
    write_const(&mut file, "FULL_VERSION", &full_version);

    println!("cargo:rustc-env=GIT_SHA={}", git_sha);
    println!("cargo:rustc-env=GIT_BRANCH={}", git_branch);
    println!("cargo:rustc-env=BUILD_DATE={}", build_date);
    println!("cargo:rustc-env=VERSION={}", version);
    println!("cargo:rustc-env=FULL_VERSION={}", full_version);
    println!("cargo:rerun-if-changed=build.rs");

    if let Some(git_dir) = run_command("git", &["rev-parse", "--git-dir"]) {
        println!("cargo:rerun-if-changed={}/HEAD", git_dir);

        if let Some(head_ref) = run_command("git", &["symbolic-ref", "-q", "HEAD"]) {
            println!("cargo:rerun-if-changed={}/{}", git_dir, head_ref);
        }
    }
}
