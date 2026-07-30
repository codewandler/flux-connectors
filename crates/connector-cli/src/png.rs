//! Rasterizing the README snippet to PNG, by asking the `flux` binary to do it (C-45).
//!
//! # Why a subprocess, and why it is opt-in
//!
//! flux-tools already rasterizes: `flux render -o out.png` renders a `.flux` file through the same
//! [`flux_lang::highlight`](connector_flux::highlight) walk this crate's SVG renderer uses, behind a
//! `png` feature carrying resvg/usvg/tiny-skia/fontdb. The alternative was to take those four crates
//! here directly. It was rejected on two grounds: flux-tools pins them as a coupled set, so this
//! repo would be maintaining a second pin of a rasterizer stack for **one** README image; and
//! `AGENTS.md` records that flux-connectors depends on `flux-lang` and nothing else — a dependency
//! is a reviewed decision, and a convenience raster does not earn one.
//!
//! What the subprocess costs is honesty about two things, both of which are why the PNG is **not**
//! a planned artifact and `build` does not write it unless asked:
//!
//! - **It is not reproducible from committed bytes.** The output depends on which `flux` is
//!   installed. Putting it in [`crate::pipeline::plan`] would make `diff` report an image stale
//!   because the developer's toolchain moved, which is exactly the noise a checked artifact must
//!   not generate. The checked README assets are the SVGs.
//! - **It is flux's rendering, not ours.** `flux render` uses flux's own palette and window chrome,
//!   so the PNG does not match `assets/readme-snippet-{light,dark}.svg` pixel for pixel. The
//!   classification underneath is identical — the same CST walk — but the presentation is flux's.
//!   The README shows the SVGs; the PNG is for surfaces that cannot display one.

use std::path::PathBuf;
use std::process::Command;

use anyhow::{bail, Context, Result};

use crate::workspace::Workspace;

/// The binary that does the rasterizing.
const FLUX: &str = "flux";

/// What a PNG request did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The raster was written to this path.
    Written(PathBuf),
    /// Nothing was written, for this reason.
    Skipped(String),
}

/// Rasterize `assets/readme-snippet.flux` to `assets/readme-snippet.png`.
///
/// Skips — rather than failing — when `flux` is not installed or there is no snippet to render:
/// the PNG is a convenience, and a build must not start depending on a binary this repo does not
/// ship. A `flux` that *is* present and fails is a real error and is reported as one.
pub fn render(workspace: &Workspace) -> Result<Outcome> {
    let snippet = workspace.snippet_path();
    if !snippet.exists() {
        return Ok(Outcome::Skipped(format!(
            "no {} to render",
            workspace.display_path(&snippet).display()
        )));
    }
    let out = workspace.snippet_png_path();

    // Relative paths, run from the repository root: flux confines both reads and writes to its
    // workspace, and an absolute path into a directory it did not start in is refused.
    let output = Command::new(FLUX)
        .current_dir(workspace.root())
        .arg("render")
        .arg("--view=source")
        .arg("--out")
        .arg(workspace.display_path(&out))
        .arg(workspace.display_path(&snippet))
        .output();

    let output = match output {
        Ok(output) => output,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Outcome::Skipped(format!(
                "`{FLUX}` is not on PATH, so {} was not written; the README uses the SVGs, which \
                 every build renders",
                workspace.display_path(&out).display()
            )));
        }
        Err(error) => {
            return Err(error).with_context(|| format!("cannot run `{FLUX} render`"));
        }
    };

    if !output.status.success() {
        bail!(
            "`{FLUX} render` failed ({}): {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(Outcome::Written(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A workspace with no snippet skips instead of shelling out — the fixture case, and the reason
    /// a PNG request can never make a build fail on a tree that has no README image.
    #[test]
    fn a_workspace_without_a_snippet_skips() {
        let workspace = Workspace::new(std::env::temp_dir().join("flux-connectors-png-absent"));
        match render(&workspace).expect("an absent snippet is not an error") {
            Outcome::Skipped(reason) => assert!(
                reason.contains("readme-snippet.flux"),
                "the skip must name what was missing, got: {reason}"
            ),
            Outcome::Written(path) => panic!("nothing to render, yet it wrote {}", path.display()),
        }
    }
}
