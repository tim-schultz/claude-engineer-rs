use std::fs;
use std::io::{self, Read};
use std::process::Command;

fn main() -> io::Result<()> {
    // Create a new text file
    let file_path = "text.txt";
    fs::write(file_path, "")?;

    // Attempt to open the file with an editor
    let formatted_path = format!("./{}", file_path);
    println!("Attempting to open file: {}", formatted_path);

    let editors = ["vim"];

    for editor in editors.iter() {
        match Command::new(editor).arg(&formatted_path).status() {
            Ok(status) => {
                println!("{} editor exited with status: {}", editor, status);
                break; // Exit the loop if an editor succeeds
            }
            Err(e) => {
                eprintln!("Failed to open {} editor: {}", editor, e);
                if editor == editors.last().unwrap() {
                    println!("No suitable editor found. Skipping edit step.");
                }
            }
        }
    }

    // Read the contents of the file after it's closed
    let mut file = fs::File::open(file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    // Print the contents to the console
    println!("File contents:");
    println!("{}", contents);

    Ok(())
}
