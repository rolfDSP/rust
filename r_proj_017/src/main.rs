fn ownership() {
    let numbers = vec![1 , 2 , 3];
    let slice = &numbers[..];
    println!("Slice: {:?}", slice);
}

fn modifiable() {
    let mut numbers = vec![1, 2, 3];
    let slice = &mut numbers[..];
    slice[0] = 10;

    println!("Slice: {:?}", slice);
}

fn main() {
    println!("Hello, r_proj_017!");

    ownership();
    modifiable();

    let v = vec![1,2,3,4,5];
    let third_v = v[2];
    println!("Third value: {}", third_v);

    let last_v = v.last().unwrap(); // unwrap: Get the value of an Option object, because the vector can be empty and None is returned
    println!("The last value is: {}", last_v);

    // Evaluate the Option as result of first()
    match v.first() {
        Some(first_v) => println!("First value is: {}", first_v),
        None => println!("Vector is empty"),
    }

    let mut v1 =  vec![1, 2, 3];
    v1.push(4);

    let more_n = vec![5, 6];
    v1.extend(more_n);
    println!("v1: {:?}", v1);

    let mut other_v = vec![7, 8];
    v1.append(&mut other_v);
    println!("v1: {:?}", v1);

    v1.insert(0,0);
    println!("v1: {:?}", v1);
}
