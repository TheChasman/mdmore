use mdmore::document::{highlight, parser, renderer};

#[test]
fn no_code_blocks_does_not_load_grammars() {
    // Own binary/process, so the lazy OnceLock state is observed reliably.
    let _ = renderer::render(&parser::parse(
        "# Title\n\nJust prose, a [link](./x.md) and `inline code`.",
    ));
    assert!(!highlight::grammars_loaded());
}
