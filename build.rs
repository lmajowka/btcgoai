use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Copy the kernel file to OUT_DIR so it can be found at runtime
    let out_dir = env::var("OUT_DIR").unwrap();
    let kernel_dest_path = Path::new(&out_dir).join("crypto_kernels.cl");
    
    // Try to copy from source directory
    let kernel_src_path = Path::new("src").join("crypto_kernels.cl");
    if kernel_src_path.exists() {
        match fs::copy(&kernel_src_path, &kernel_dest_path) {
            Ok(_) => println!("cargo:warning=Copied kernel file to output directory"),
            Err(e) => println!("cargo:warning=Failed to copy kernel file: {}", e),
        }
    } else {
        println!("cargo:warning=Kernel source file not found at {:?}", kernel_src_path);
    }

    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/crypto_kernels.cl");
    
    // Set the name of the output directory as an environment variable
    println!("cargo:rustc-env=KERNEL_OUT_DIR={}", out_dir);
} 