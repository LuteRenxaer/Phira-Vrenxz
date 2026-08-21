use inputbox::{InputBox, InputMode};

fn main() {
    let input = InputBox::new()
        .title("Title")
        .prompt("Enter something")
        .default_text("Default value")
        .mode(InputMode::Text)
        .width(400)
        .height(200)
        .cancel_label("Nope")
        .ok_label("Fine");

    let result = input.show();
    println!("Result: {:?}", result);
}
