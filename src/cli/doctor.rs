use std::process::Command;

use devclean::{config_candidates, default_roots};

pub(super) fn run() {
    println!("devclean {}", env!("CARGO_PKG_VERSION"));
    println!("default roots:");
    for root in default_roots() {
        println!("  {}", root.display());
    }
    println!("config search:");
    for path in config_candidates() {
        println!(
            "  {} {}",
            if path.is_file() {
                "loaded"
            } else {
                "candidate"
            },
            path.display()
        );
    }
    println!("tools:");
    for tool in ["cargo", "docker", "git", "npm", "pnpm"] {
        println!(
            "  {tool:<8} {}",
            if command_exists(tool) {
                "available"
            } else {
                "not found"
            }
        );
    }
    println!("safety:");
    println!("  scan is always read-only");
    println!("  clean requires confirmation or --yes");
    println!("  Git-tracked files are protected unless --allow-tracked is explicit");
    println!("  candidates are atomically quarantined before recursive deletion");
    println!("  symlinks, VCS metadata, backups, databases and volumes are protected");
    println!("  --docker prunes build cache only; --docker-system never includes volumes");
}

fn command_exists(command: &str) -> bool {
    Command::new(command)
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}
