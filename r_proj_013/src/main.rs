// function not returning anything
fn print_sum(numbers: & [i32]) {
    let sum : i32 = numbers.iter().sum();
    if sum % 2 == 0 {
        println!("Sum is even");
    } else {
        println!("Sum is odd");
    }
}

fn split_string(s: String, delimiter: char, field: usize) -> String {
    let parts: Vec<&str> = s.split(delimiter).collect();
    let result: Option<&&str> = parts.get(field);

    result.expect("Something went wrong..").to_string()
}

fn main() {
    println!("Hello, r_proj_13!");

    let v = [1, 5, 8];
    print_sum(&v);

    let s1 = "Hello, world, how are you?".to_string();
    let chunk = split_string(s1, ',', 1);
    println!("Split string: {}", chunk);
}
