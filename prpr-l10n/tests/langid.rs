use prpr_l10n::Lazy;

#[test]
fn check_langid() {

    Lazy::force(&prpr_l10n::LANG_IDENTS);
}
