#[derive(Debug)]

struct Person {
    first_name: String,
    last_name: String,
    age: u8,
    membership: Option<u8>,
}

struct User {
    username: String,
    url: String,
    email: String,
    active: bool,
}

impl User {
    fn new(username: String, url: String, email: String) -> User {
        User {
            username,
            url,
            email,
            active: true,
        }
    }

    fn print(&self) {
        println!("username: {}", self.username);
        println!("url: {}", self.url);
        println!("email: {}", self.email);
        println!("active: {}", yes_no(self.active));
    }

    fn deactivate(&mut self) {
        self.active = false;
    }
}

fn yes_no(d : bool) -> String {
    if d {
        "Yes".to_string()
    } else {
        "No".to_string()
    }
}

struct Point ( i32, i32, i32 );
impl Point {
    fn sum(&self) -> i32 {
        self.0 + self.1 + self.2
    }
}

fn main() {
    println!("Hello, r_proj_015!");

    let myname = Person {
        first_name: "Rolf".to_string(),
        last_name: "Maier".to_string(),
        age: 33,
        membership: Some(10u8),
    };

    println!("The person's first name is: {}", myname.first_name);
    //let mut m: u8 = 0;
    let m: u8 = match myname.membership {
        Some(m) => {
            println!("Membership time is: {}", m);
            m
        },
        None => {
            println!("No membership time defined");
            0u8
        }
    };
    println!("Membership time assumed: {}", m);

    let mut new_user = User::new(
        String::from("Annabel Jurgens"),
        String::from("https://moyenage.com"),
        String::from("jurgens@age.eu")
    );

    //println!("New user: {}", new_user.username);
    new_user.deactivate();
    new_user.print();

    let p = Point(10, 20, 30);
    println!("p[0]= {}", p.0);
    println!("Sum of p: {}", p.sum());
}
