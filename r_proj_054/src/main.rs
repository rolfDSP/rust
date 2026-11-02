struct MySuperSliceable<T: ?Sized> {
    info: u32,
    data: T,
}

fn main() {
    println!("Hello, r_proj_054!");

    let sized: MySuperSliceable<[u8; 8]> = MySuperSliceable {
        info: 17,
        data: [0; 8],
    };

    let dynamic: &MySuperSliceable<[u8]> = &sized;

    println!("{} {:?}", dynamic.info, &dynamic.data);

    let mut x: [u8; 10] = [1; 10];
    println!("x: {:?}", x);
}
