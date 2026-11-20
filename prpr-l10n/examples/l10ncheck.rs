fn main() {
    let roots = [
        concat!(env!("CARGO_MANIFEST_DIR"), "/../prpr/locales/"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/../Phira-Vrenxz/locales/"),
    ];
    let mut fail = false;
    for root in roots {
        match prpr_l10n::tools::check_langfile(root) {
            Ok(_) => println!("OK {root}"),
            Err(e) => {
                eprintln!("FAIL {root}\n{e}");
                fail = true;
            }
        }
    }
    if fail {
        std::process::exit(1);
    }
}
