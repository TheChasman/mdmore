use mdmore::document::highlight;
use mdmore::print::fits_in;

#[test]
fn fits_in_counts_without_loading_grammars() {
    assert!(fits_in("# T\n\n```rust\nfn a() {}\n```\n", 80, 100));
    assert!(!fits_in("# T\n\n```rust\nfn a() {}\n```\n", 80, 1));
    assert!(!highlight::grammars_loaded());
}
