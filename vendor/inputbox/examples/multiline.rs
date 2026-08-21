use inputbox::{InputBox, InputMode};

fn main() {
    let input = InputBox::new()
        .default_text("Multiline\nInput\nBox")
        .mode(InputMode::Multiline);

    let result = input.show();
    println!("Result: {:?}", result);
}
