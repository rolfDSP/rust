fn print_str(s: &str) {
    println!("{}", s);
}

fn print_string(s: String) {
    println!("{}", s);
}

fn main() {
    println!("Hello, r_proj_016!");

    let s1 = "Hello, slice";
    print_str(s1);

    let n_s1 = format!("The string is: {}", s1);
    println!("{}", n_s1);

    let s2 = String::from("Hello, String");
    print_string(s2);

    let sentence = "All the good things that happened to me are important";
    println!("{}", &sentence[0..4]);
    println!("{}", &sentence[0..=4]);

    for c in sentence.chars() {
        match c {
            'a' | 'e' | 'i' | 'o' | 'u' => println!("Got a vowel"),
            _ => continue,
        }
    }

    let words : Vec<&str> = sentence.split_whitespace().collect();
    println!("{:?}", words);

    let reverse = sentence.chars().rev().collect::<String>();
    println!("{}", reverse);
}
