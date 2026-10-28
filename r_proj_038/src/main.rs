/// Rust iterator examples
///
fn heading(s: String) {
    println!("\n{:?}",s);
    println!("===============================");
}
fn example1() {
    heading("Example1".to_string());
    let vec1 = vec![1, 2, 3, 4, 5];

    for x in vec1.iter() {
        println!("vec1 contained: {x:?}");
    }
}

fn example2() {
    heading("Example 2".to_string());
    let mut vec1 = vec![1, 4, 6, 10, 3];

    for x in vec1.iter_mut() {
        *x += 1;
    }

    for x in vec1.iter() {
        println!("vec1 contained (2): {x:?}");
    }
}

fn example3() {
    heading("Example 3". to_string());
    let mut vec1 = vec![10, 20, 30, 40];
    let vec2 = vec![1, 2, 3, 4];
    vec1.extend(vec2);
    println!("vec1 (3): {:?}", vec1);
}

fn example4() {
    use std::collections::VecDeque;

    heading("Example 4".to_string());
    let vec = vec![2,4,6,8];
    let buf: VecDeque<i32> = vec.into_iter().collect();
    println!("buf: (4): {:?}", buf);
}

fn example5() {
    heading("Example 5".to_string());

    let vec1 = vec![2, 5, 1, 8, 9];
    for x in vec1.iter().rev() {
        println!("vec1: El: {:?}", x);
    }
}

fn main() {
    println!("Hello, r_proj_038!");

    example1();
    example2();
    example3();
    example4();
    example5();
}
