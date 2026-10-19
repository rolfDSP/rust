use std::fs::File;

enum DiskType {
    ssd,
    hdd,
}

#[derive(Debug)]
enum DiskSize {
    KB(u32),
    MB(u32),
    GB(u32),
}

enum Material {
    Gold,
    Silver,
    Platinum,
}

struct Device {
    material: Material,
    weight: f32,
}

fn printDevice(d : Device) {
    match d.material {
        Material::Gold => println!("Material is Gold"),
        Material::Silver => println!("Material is Silver"),
        Material::Platinum => println!("Material is Platinum"),
    }

    println!("The weight is: {}", d.weight);
}

enum FileSize {
    Bytes(u64),
    Kilobytes(u64),
    Megabytes(u64),
    Gigabytes(u64),
}

impl FileSize {
    fn format_size(size: u64) -> String {
        let filesize = match size {
            0..=999 => FileSize::Bytes(size),
            1000..=999_999 => FileSize::Kilobytes(size / 1000),
            1_000_000..=999_999_999 => FileSize::Megabytes(size / 1_000_000),
            _ => FileSize::Gigabytes(size / 1_000_000_000),
        };

        match filesize {
            FileSize::Bytes(bytes) => format!("{} bytes", bytes),
            FileSize::Kilobytes(kb) => format!("{:.2} kilobytes", kb as f64),
            FileSize::Megabytes(mb) => format!("{:.2} megabytes", mb as f64),
            FileSize::Gigabytes(gb) => format!("{:.2} gigabytes", gb as f64),
        }
    }
}

fn main() {
    println!("Hello, r_proj_018!");

    let disk_type = DiskType::hdd;

    // comparison of enum with match, not ==
    match disk_type {
        DiskType::ssd => println!("Disk Type SSD"),
        DiskType::hdd => println!("Disk Type HDD"),
    }

    let disk_size = DiskSize::GB(10u32);
    println!("Disk size is: {:?}", disk_size);

    let goldbar = Device {
        material: Material::Gold,
        weight: 240.0,
    };

    printDevice( goldbar );

    let fmt = FileSize::format_size(1000000000 as u64);
    println!("Formatted size: {}", fmt);
}
