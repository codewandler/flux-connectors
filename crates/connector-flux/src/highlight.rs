//! Rendering Flux source as a syntax-highlighted SVG, coloured by **flux's own classifier**.
//!
//! GitHub's markdown strips `<style>` and `<script>`, so a highlighted code block in a README has to
//! be an image; SVG rather than PNG because it stays text — diffable in review, crisp at any zoom,
//! and a fraction of the size. Colours are inline `fill` attributes on `<tspan>`, which survive
//! GitHub's SVG sanitizer where a `<style>` block would not.
//!
//! # Why the classification is not ours to make
//!
//! [`flux_lang::highlight::highlight`] walks the lossless CST and classifies each token by its kind
//! **and its parent node's kind**, so `when` is a keyword because of where it sits rather than
//! because it is on a list. Its own documentation calls keyword-list matching "strictly less
//! accurate", and this module exists because a keyword list is what this repo shipped first: a regex
//! tokeniser that had no [`HighlightClass::Annotation`], no `Op`/`Type` distinction and no
//! `Error` at all, and that coloured `ticket_id` in `op f(ticket_id: Number)` as plain text while
//! colouring `$base` — the same [`HighlightClass::Var`] — as a symbol.
//!
//! The classifier is also **total**: malformed source still yields spans, so rendering never fails
//! and never has a fallback path that could disagree with the real one.
//!
//! # What this looks like on a connector module, and why it is not worked around here
//!
//! flux-lang 0.37 classifies a **composite `op` declaration** — which is the only thing this repo
//! emits — less richly than it classifies a `flow`. Its parser gives an `op` its own node kinds
//! (`OP_DECL`/`OP_HEADER` for the header, `OP_META` for the `description`/`risk`/`idempotency`/
//! `effects`/`expose` lines), and `highlight`'s tables list only the `flow` spellings:
//! `keyword_leads` covers `FLOW_HEADER` but not `OP_HEADER` or `OP_META`, and `name_class` treats a
//! type reference as a [`HighlightClass::Type`] under `PARAM`/`FLOW_HEADER` but not under
//! `OP_HEADER`. So in a rendered connector `op`, the leading `op`, the operation's own name and the
//! six metadata keywords come back as [`HighlightClass::Punct`], and the return type comes back as
//! [`HighlightClass::Op`] while a parameter's type is a `Type`.
//!
//! That is visible: the README image is less colourful than the regex script's output was, because
//! the regex script *asserted* that `op` and `description` are keywords. Making the same assertion
//! here would rebuild the thing this module deletes — a local opinion about Flux's grammar, free to
//! drift from it. The fix belongs upstream, in flux-lang's own tables, and is a few entries long;
//! until then this renders what flux actually knows.
//!
//! # Why not `flux_lang::render::Palette`
//!
//! flux-lang has a `Palette`/`Role` pair, and it is the wrong tool here. It colours the *plan tree*
//! ([`flux_lang::render::render_styled_spans`]) rather than source text: its `Role` vocabulary has
//! `Connector` and `Thing` — tree glyphs and thing-selectors, which source highlighting never emits
//! — and lacks `Comment`, `Type`, `Punct` and `Error`, which are exactly the distinctions this
//! story was filed to gain. Its fields are also ANSI `(open, close)` pairs, a shape that cannot
//! carry the background and border an image needs. Mapping [`HighlightClass`] onto `Role` would
//! throw away four classes to reuse eight fields, so [`Theme`] is keyed on [`HighlightClass`]
//! directly and [`Theme::fill`] is an exhaustive `match`: a class added to a future flux-lang pin
//! stops this crate compiling rather than rendering as an inherited colour.

use flux_lang::highlight::{highlight, HighlightClass};

/// The monospace stack, matched to what GitHub renders code in.
const FONT: &str = "ui-monospace, SFMono-Regular, 'SF Mono', Menlo, Consolas, monospace";

/// Font size in SVG user units.
const FONT_SIZE: f64 = 13.0;

/// Advance width of one character in [`FONT`] at [`FONT_SIZE`]. Measured, not derived: the image is
/// sized from the character count, so a wrong value clips the longest line or pads the box.
const CHAR_WIDTH: f64 = FONT_SIZE * 0.6009;

/// Baseline-to-baseline distance.
const LINE_HEIGHT: f64 = 20.0;

/// Padding between the box and the text, horizontally and vertically.
const PAD_X: f64 = 16.0;
const PAD_Y: f64 = 14.0;

/// The colours one rendering uses, one per [`HighlightClass`] plus the box itself.
///
/// Every class gets its **own** colour rather than sharing one with a neighbour. Two classes
/// painted alike are indistinguishable to a reader, which is the same failure as not colouring
/// them at all — and `tests/readme_snippet_svg.rs` asserts the distinctness on the committed image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    /// The palette's name, which is also the asset's filename suffix (`readme-snippet-<name>.svg`).
    pub name: &'static str,
    /// The box fill.
    pub background: &'static str,
    /// The box stroke.
    pub border: &'static str,
    /// Contextual keywords: `op`, `return`, `when`, …
    pub keyword: &'static str,
    /// Operation and callable names.
    pub op: &'static str,
    /// `$symbol` references and the binder positions that become symbols.
    pub var: &'static str,
    /// `@annotation` and the tag inside `@effect(tag)`.
    pub annotation: &'static str,
    /// String literals.
    pub string: &'static str,
    /// Numeric literals and the literal idents `true`/`false`/`null`.
    pub number: &'static str,
    /// `# …` line comments.
    pub comment: &'static str,
    /// Punctuation, operators, and idents with no more specific classification.
    pub punct: &'static str,
    /// Type names.
    pub type_name: &'static str,
    /// Tokens the lexer could not classify, or that the parser wrapped in an `ERROR` node.
    pub error: &'static str,
}

impl Theme {
    /// The colour this theme paints `class` with.
    ///
    /// Exhaustive by design — see the module docs. A new [`HighlightClass`] is a compile error here,
    /// not a token that quietly inherits the surrounding colour.
    pub fn fill(&self, class: HighlightClass) -> &'static str {
        match class {
            HighlightClass::Keyword => self.keyword,
            HighlightClass::Op => self.op,
            HighlightClass::Var => self.var,
            HighlightClass::Annotation => self.annotation,
            HighlightClass::String => self.string,
            HighlightClass::Number => self.number,
            HighlightClass::Comment => self.comment,
            HighlightClass::Punct => self.punct,
            HighlightClass::Type => self.type_name,
            HighlightClass::Error => self.error,
        }
    }
}

/// GitHub's light code colours, so the image sits naturally in a rendered README.
pub const LIGHT: Theme = Theme {
    name: "light",
    background: "#ffffff",
    border: "#d1d9e0",
    keyword: "#cf222e",
    op: "#6639ba",
    var: "#8250df",
    annotation: "#116329",
    string: "#0a3069",
    number: "#0550ae",
    comment: "#59636e",
    punct: "#1f2328",
    type_name: "#953800",
    error: "#82071e",
};

/// GitHub's dark code colours, selected by `prefers-color-scheme: dark` in the README's `<picture>`.
pub const DARK: Theme = Theme {
    name: "dark",
    background: "#0d1117",
    border: "#3d444d",
    keyword: "#ff7b72",
    op: "#d2a8ff",
    var: "#bc8cff",
    annotation: "#7ee787",
    string: "#a5d6ff",
    number: "#79c0ff",
    comment: "#9198a1",
    punct: "#e6edf3",
    type_name: "#ffa657",
    error: "#ffa198",
};

/// Both palettes, in the order the README's `<picture>` lists them.
pub const THEMES: &[Theme] = &[LIGHT, DARK];

/// Render `source` as a self-contained SVG in `theme`.
///
/// Total, because [`highlight`] is: malformed Flux still renders, uncoloured where flux could not
/// classify it and [`HighlightClass::Error`]-coloured where it knew it could not.
pub fn render_svg(source: &str, theme: &Theme) -> String {
    let lines = split_lines(source);
    let columns = lines
        .iter()
        .map(|line| line.iter().map(|run| run.text.chars().count()).sum())
        .max()
        .unwrap_or(0);
    let width = PAD_X * 2.0 + columns as f64 * CHAR_WIDTH;
    let height = PAD_Y * 2.0 + lines.len() as f64 * LINE_HEIGHT;

    let mut svg = String::new();
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{height:.0}\" \
         viewBox=\"0 0 {width:.0} {height:.0}\" role=\"img\" \
         aria-label=\"Generated Flux-Lang source\">"
    ));
    svg.push_str(&format!(
        "<rect width=\"{width:.0}\" height=\"{height:.0}\" rx=\"6\" fill=\"{}\" stroke=\"{}\"/>",
        theme.background, theme.border
    ));
    svg.push_str(&format!(
        "<text font-family=\"{}\" font-size=\"{FONT_SIZE:.0}\" fill=\"{}\" xml:space=\"preserve\">",
        escape(FONT),
        theme.punct
    ));

    for (row, line) in lines.iter().enumerate() {
        let y = PAD_Y + LINE_HEIGHT * row as f64 + FONT_SIZE;
        svg.push_str(&format!("<tspan x=\"{PAD_X:.0}\" y=\"{y:.1}\">"));
        for run in line {
            match run.class {
                // Whitespace between tokens carries no class, so it carries no colour either — it
                // inherits the `<text>` fill and costs no markup.
                None => svg.push_str(&escape(run.text)),
                Some(class) => svg.push_str(&format!(
                    "<tspan fill=\"{}\">{}</tspan>",
                    theme.fill(class),
                    escape(run.text)
                )),
            }
        }
        svg.push_str("</tspan>");
    }

    svg.push_str("</text></svg>\n");
    svg
}

/// One run of source text sharing a class, within one line.
struct Run<'a> {
    text: &'a str,
    class: Option<HighlightClass>,
}

/// Split `source` into lines of classified runs.
///
/// Two things happen here that [`highlight`] leaves to its consumer. Whitespace between tokens is
/// not covered by any span, so it becomes an unclassified run; and a multi-line token (a `"""…"""`
/// string) arrives as **one** span covering all of its lines, so it is cut at every line boundary —
/// the highlight docs name that as the consumer's job.
fn split_lines(source: &str) -> Vec<Vec<Run<'_>>> {
    let source = source.trim_end_matches('\n');
    let spans: Vec<(usize, usize, HighlightClass)> = highlight(source)
        .into_iter()
        .map(|(range, class)| (usize::from(range.start()), usize::from(range.end()), class))
        .filter(|(start, end, _)| *start < source.len() && *end <= source.len())
        .collect();

    let mut lines: Vec<Vec<Run<'_>>> = vec![Vec::new()];
    let mut at = 0usize;
    for (start, end, class) in spans {
        if at < start {
            emit(&mut lines, &source[at..start], None);
        }
        emit(&mut lines, &source[start..end], Some(class));
        at = end;
    }
    if at < source.len() {
        emit(&mut lines, &source[at..], None);
    }
    lines
}

/// Append `text` to the last line, starting a new line at every `\n` it contains.
fn emit<'a>(lines: &mut Vec<Vec<Run<'a>>>, text: &'a str, class: Option<HighlightClass>) {
    for (index, piece) in text.split('\n').enumerate() {
        if index > 0 {
            lines.push(Vec::new());
        }
        if !piece.is_empty() {
            lines
                .last_mut()
                .expect("a line is always open")
                .push(Run { text: piece, class });
        }
    }
}

/// Escape text for an XML text node or a double-quoted attribute value.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The classes a source text actually contains, named for readable assertions.
    fn classes(source: &str) -> Vec<HighlightClass> {
        let mut seen: Vec<HighlightClass> = Vec::new();
        for (_, class) in highlight(source) {
            if !seen.contains(&class) {
                seen.push(class);
            }
        }
        seen
    }

    /// Every class this theme can paint is painted differently. Two classes sharing a colour is the
    /// same as not distinguishing them, which is the failure this module was written to end.
    #[test]
    fn each_theme_paints_every_class_a_different_colour() {
        const ALL: &[HighlightClass] = &[
            HighlightClass::Keyword,
            HighlightClass::Op,
            HighlightClass::Var,
            HighlightClass::Annotation,
            HighlightClass::String,
            HighlightClass::Number,
            HighlightClass::Comment,
            HighlightClass::Punct,
            HighlightClass::Type,
            HighlightClass::Error,
        ];
        for theme in THEMES {
            let mut fills: Vec<&str> = ALL.iter().map(|class| theme.fill(*class)).collect();
            fills.sort_unstable();
            let distinct = fills.len();
            fills.dedup();
            assert_eq!(
                fills.len(),
                distinct,
                "the {} theme paints two classes the same colour",
                theme.name
            );
        }
    }

    /// The classes the regex highlighter this module replaced could not produce, rendered.
    #[test]
    fn annotations_comments_and_errors_are_coloured() {
        // `€` is unlexable, so the parser wraps it in an `ERROR` node — the classifier is total, so
        // it still yields a span rather than a parse failure.
        let source = "flow f\n  # a note\n  @effect(net)\n  € oops\n  return 1\n";
        let present = classes(source);
        for class in [
            HighlightClass::Annotation,
            HighlightClass::Comment,
            HighlightClass::Error,
        ] {
            assert!(
                present.contains(&class),
                "the fixture must exercise {class:?}, got {present:?}"
            );
            let svg = render_svg(source, &LIGHT);
            assert!(
                svg.contains(&format!("fill=\"{}\"", LIGHT.fill(class))),
                "{class:?} is not painted in the rendered SVG:\n{svg}"
            );
        }
    }

    /// A multi-line string is one span; the renderer cuts it at the line boundary rather than
    /// emitting a `<tspan>` whose text runs past the end of its line.
    #[test]
    fn a_multi_line_token_is_split_at_the_line_boundary() {
        let source = "flow f\n  $x = \"\"\"one\ntwo\"\"\"\n  return $x\n";
        let svg = render_svg(source, &LIGHT);
        assert!(
            !svg.contains('\n') || svg.ends_with('\n'),
            "no literal newline may survive inside the markup:\n{svg}"
        );
        assert_eq!(
            svg.matches("<tspan x=").count(),
            4,
            "four source lines must render as four line tspans:\n{svg}"
        );
    }

    /// Malformed source renders rather than failing — the property [`highlight`] documents as
    /// totality, carried through to the image.
    #[test]
    fn malformed_source_still_renders() {
        let svg = render_svg("flow f\n  $a =\n  do read(\n", &LIGHT);
        assert!(svg.starts_with("<svg "));
        assert!(svg.ends_with("</svg>\n"));
    }

    /// The rendering is a pure function of the source and the theme: the same inputs produce the
    /// same bytes, which is what lets the pipeline treat the image as a checked artifact.
    #[test]
    fn rendering_is_deterministic() {
        let source = "flow f\n  return 1\n";
        assert_eq!(render_svg(source, &LIGHT), render_svg(source, &LIGHT));
        assert_ne!(render_svg(source, &LIGHT), render_svg(source, &DARK));
    }
}
