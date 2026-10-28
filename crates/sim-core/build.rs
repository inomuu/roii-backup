// Embed the git identity of the SOURCE this sim was built from, so the client
// dylib and the deployed server can be compared by COMMIT, not just by the
// hand-bumped VERSION string (two builds can both claim "0.0.X-alpha" while
// coming from different trees — the exact skew that once shipped a silent
// combat diff). tools/deploy.sh + check_online_sync.sh compare these.
//
// Resolution order:
//   1. SIM_GIT_SHA env (Docker build-arg — .dockerignore strips .git/)
//   2. `git rev-parse` + a `-dirty` suffix when BUILD-AFFECTING paths
//      (crates/ data/ Cargo.*) have uncommitted changes
//   3. "unknown"
use std::process::Command;

fn main() {
    // Re-stamp when the checked-out commit moves.
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");
    // …and when the SOURCE moves without a commit or `git add` (audit 2026-10-02): an unstaged edit
    // changed neither file above, so the stamp kept a clean SHA for code that no longer matched it
    // and the release gate passed a client the server would flag. Directories are scanned
    // recursively. Only what reaches the dylib / sim — a server-only edit does not rebuild the sim.
    println!("cargo:rerun-if-changed=../../crates/sim-core/src");
    println!("cargo:rerun-if-changed=../../crates/sim-core/Cargo.toml");
    println!("cargo:rerun-if-changed=../../crates/sim-host/src");
    println!("cargo:rerun-if-changed=../../crates/sim-ffi/src");
    println!("cargo:rerun-if-changed=../../Cargo.lock");
    println!("cargo:rerun-if-env-changed=SIM_GIT_SHA");

    let sha = std::env::var("SIM_GIT_SHA").ok().filter(|s| !s.is_empty()).unwrap_or_else(|| {
        let rev = Command::new("git")
            .args(["rev-parse", "--short=12", "HEAD"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        match rev {
            Some(r) if !r.is_empty() => {
                // Dirty only counts for paths that reach the server image / dylib.
                let dirty = Command::new("git")
                    .args(["status", "--porcelain", "--", "crates", "data", "Cargo.toml", "Cargo.lock"])
                    .current_dir("../..")
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .map(|o| !o.stdout.is_empty())
                    .unwrap_or(false);
                if dirty { format!("{r}-dirty") } else { r }
            }
            _ => "unknown".into(),
        }
    });
    println!("cargo:rustc-env=SIM_GIT_SHA={sha}");

    // SIM BUILD HASH (audit 2026-10-03): a digest of what the REPLAY runs — sim-core's own source —
    // so the save gate can tell "this client simulates exactly like this server" apart from "this
    // server was redeployed". The git SHA moved with every server-only or docs-only deploy and
    // refused every shipped client. Line endings normalised (a Windows checkout builds the same).
    println!("cargo:rustc-env=SIM_BUILD_HASH={}", sim_build_hash());
}

/// FNV-1a 64 twice (two offsets) over sim-core's NON-TEST `.rs` files plus its Cargo.toml, in
/// sorted path order, path then content. Code only (audit 2026-10-03): comments are stripped and
/// whitespace runs collapsed, so a changelog line in `lib.rs` or a comment fix does not refuse every
/// shipped client, and stray files (`.DS_Store`, editor backups) never count. `tests.rs` files and
/// `tests/` directories are skipped; inline `#[cfg(test)]` modules still count.
fn sim_build_hash() -> String {
    fn walk(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path();
                let name = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
                if p.is_dir() {
                    if name != "tests" {
                        walk(&p, out);
                    }
                } else if name.ends_with(".rs") && name != "tests.rs" {
                    out.push(p);
                }
            }
        }
    }
    let root = std::path::Path::new(".");
    let mut files = Vec::new();
    walk(&root.join("src"), &mut files);
    let mut rel: Vec<(String, std::path::PathBuf)> = files.into_iter()
        .map(|p| (p.strip_prefix(root).unwrap_or(&p).to_string_lossy().replace('\\', "/"), p))
        .collect();
    rel.sort();
    let (mut a, mut b): (u64, u64) = (0xcbf2_9ce4_8422_2325, 0x8422_2325_cbf2_9ce4);
    let mut feed = |bytes: &[u8]| {
        for &x in bytes {
            a = (a ^ u64::from(x)).wrapping_mul(0x0000_0100_0000_01b3);
            b = (b ^ u64::from(x)).wrapping_mul(0x0000_0100_0000_01b3);
        }
    };
    for (name, path) in rel {
        feed(name.as_bytes());
        feed(&[0]);
        feed(code_only(&String::from_utf8_lossy(&std::fs::read(&path).unwrap_or_default())).as_bytes());
        feed(&[0]);
    }
    let toml = String::from_utf8_lossy(&std::fs::read(root.join("Cargo.toml")).unwrap_or_default()).replace('\r', "");
    feed(toml.as_bytes());
    format!("{a:016x}{b:016x}")
}

/// Rust source with comments removed and every whitespace run outside a literal collapsed to one
/// space. String, raw-string and char literals are copied verbatim; a `'` that does not open a char
/// literal is a lifetime.
fn code_only(src: &str) -> String {
    let c: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut pending_space = false;
    let push = |out: &mut String, pending: &mut bool, s: &[char]| {
        if *pending && !out.is_empty() {
            out.push(' ');
        }
        *pending = false;
        out.extend(s.iter());
    };
    while i < c.len() {
        let ch = c[i];
        let next = c.get(i + 1).copied();
        if ch == '/' && next == Some('/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            pending_space = true;
        } else if ch == '/' && next == Some('*') {
            let mut depth = 0usize;
            while i < c.len() {
                if c[i] == '/' && c.get(i + 1) == Some(&'*') {
                    depth += 1;
                    i += 2;
                } else if c[i] == '*' && c.get(i + 1) == Some(&'/') {
                    depth = depth.saturating_sub(1);
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            pending_space = true;
        } else if ch.is_whitespace() {
            pending_space = true;
            i += 1;
        } else if ch == 'r' && (next == Some('"') || next == Some('#'))
            && {
                // `r` opens a raw string unless it ends an identifier; `br` is a byte raw string.
                let ident = |k: usize| c.get(k).is_some_and(|p| p.is_alphanumeric() || *p == '_');
                let prev = i.checked_sub(1);
                match prev {
                    None => true,
                    Some(p) if c[p] == 'b' => !p.checked_sub(1).is_some_and(ident),
                    Some(p) => !ident(p),
                }
            }
            && { let mut j = i + 1; while c.get(j) == Some(&'#') { j += 1; } c.get(j) == Some(&'"') }
        {
            let mut j = i + 1;
            let mut hashes = 0;
            while c.get(j) == Some(&'#') { hashes += 1; j += 1; }
            j += 1; // opening quote
            loop {
                if j >= c.len() { break; }
                if c[j] == '"' && (0..hashes).all(|k| c.get(j + 1 + k) == Some(&'#')) {
                    j += 1 + hashes;
                    break;
                }
                j += 1;
            }
            let end = j.min(c.len());
            push(&mut out, &mut pending_space, &c[i..end]);
            i = end;
        } else if ch == '"' {
            let mut j = i + 1;
            while j < c.len() && c[j] != '"' {
                if c[j] == '\\' { j += 1; }
                j += 1;
            }
            let end = (j + 1).min(c.len());
            push(&mut out, &mut pending_space, &c[i..end]);
            i = end;
        } else if ch == '\'' {
            // `'\x'`-style escape, or one char then a closing quote: a char literal.
            let end = if next == Some('\\') {
                let mut j = i + 3; // past the backslash and the char it escapes (`'\''`, `'\\'`)
                while j < c.len() && c[j] != '\'' { j += 1; }
                Some(j + 1)
            } else if c.get(i + 2) == Some(&'\'') {
                Some(i + 3)
            } else {
                None
            };
            let end = end.unwrap_or(i + 1).min(c.len());
            push(&mut out, &mut pending_space, &c[i..end]);
            i = end;
        } else {
            push(&mut out, &mut pending_space, &c[i..i + 1]);
            i += 1;
        }
    }
    out
}
