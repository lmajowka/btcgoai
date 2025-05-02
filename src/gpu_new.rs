use std::error::Error;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

// Import the opencl3 crate conditionally
#[cfg(feature = "opencl")]
use opencl3::device::{Device, CL_DEVICE_TYPE_ALL};
#[cfg(feature = "opencl")]
use opencl3::platform::Platform;
#[cfg(feature = "opencl")]
use opencl3::context::Context;
#[cfg(feature = "opencl")]
use opencl3::command_queue::{CommandQueue, CL_QUEUE_PROFILING_ENABLE};
#[cfg(feature = "opencl")]
use opencl3::program::Program;
#[cfg(feature = "opencl")]
use opencl3::kernel::Kernel;
#[cfg(feature = "opencl")]
use opencl3::memory::{Buffer, CL_MEM_READ_ONLY, CL_MEM_WRITE_ONLY};
#[cfg(feature = "opencl")]
use opencl3::types::*;

// Static flag for OpenCL availability
static OPENCL_AVAILABLE: AtomicBool = AtomicBool::new(false);

// OpenCL kernel for Bitcoin key search
const KERNEL_SRC: &str = r#"
// OpenCL kernel for Bitcoin key search

// Simple RIPEMD-160 and SHA-256 implementation for OpenCL
// This is a simplified version for demonstration purposes

// Constants for SHA-256
#define SHA256_DIGEST_SIZE 32

// Constants for RIPEMD-160
#define RIPEMD160_DIGEST_SIZE 20

// Helper functions for Bitcoin key operations
ulong rotr64(ulong x, int n) {
    return (x >> n) | (x << (64 - n));
}

uint rotr32(uint x, int n) {
    return (x >> n) | (x << (32 - n));
}

uint ch(uint x, uint y, uint z) {
    return (x & y) ^ (~x & z);
}

uint maj(uint x, uint y, uint z) {
    return (x & y) ^ (x & z) ^ (y & z);
}

uint sigma0(uint x) {
    return rotr32(x, 2) ^ rotr32(x, 13) ^ rotr32(x, 22);
}

uint sigma1(uint x) {
    return rotr32(x, 6) ^ rotr32(x, 11) ^ rotr32(x, 25);
}

uint gamma0(uint x) {
    return rotr32(x, 7) ^ rotr32(x, 18) ^ (x >> 3);
}

uint gamma1(uint x) {
    return rotr32(x, 17) ^ rotr32(x, 19) ^ (x >> 10);
}

// Function to compute Bitcoin address hash (RIPEMD160(SHA256(pubkey)))
bool compute_hash160(ulong private_key, uchar hash160[20]) {
    // This function would:
    // 1. Convert private key to public key using secp256k1
    // 2. Apply SHA-256 to the public key
    // 3. Apply RIPEMD-160 to the SHA-256 hash
    
    // In a real implementation, this would do the actual computation
    // For demonstration, we'll just set some dummy values based on the private key
    
    // Example implementation using private key bits to influence the hash
    for (int i = 0; i < 20; i++) {
        hash160[i] = (private_key >> ((i % 8) * 8)) & 0xFF;
    }
    
    return true;
}

// Compare two hash160 values
bool compare_hash160(uchar* hash1, uchar* hash2) {
    for (int i = 0; i < 20; i++) {
        if (hash1[i] != hash2[i]) {
            return false;
        }
    }
    return true;
}

// Main search kernel
__kernel void search_keys(
    __global const ulong* private_key_ranges,
    __global const uchar* target_hashes,
    __global const uint* num_targets,
    __global uint* results,
    __global ulong* found_keys
) {
    // Get global ID
    const size_t gid = get_global_id(0);
    
    // Check if this work item has a valid range
    const size_t range_idx = gid * 2;
    const ulong range_start = private_key_ranges[range_idx];
    const ulong range_end = private_key_ranges[range_idx + 1];
    
    // Skip if range is invalid
    if (range_start >= range_end) {
        return;
    }
    
    // Get the number of target hashes
    const uint targets_count = num_targets[0];
    
    // Initialize temporary buffer for hash computation
    uchar computed_hash[20];
    
    // Number of keys to check per kernel execution
    const uint keys_per_work_item = 100;
    
    // Loop through a small batch of keys in this range
    for (ulong key = range_start; key < range_end && key < range_start + keys_per_work_item; key++) {
        // Calculate hash160 for this private key
        if (compute_hash160(key, computed_hash)) {
            // Check against all targets
            for (uint t = 0; t < targets_count; t++) {
                // Compare with target hash
                bool match = true;
                for (uint i = 0; i < 20; i++) {
                    if (computed_hash[i] != target_hashes[t * 20 + i]) {
                        match = false;
                        break;
                    }
                }
                
                // If found a match
                if (match) {
                    // Atomic increment of results count
                    uint idx = atomic_inc(&results[0]);
                    
                    // Check if we have space for this result
                    if (idx < 100) {  // Max 100 results
                        // Store the private key
                        found_keys[idx] = key;
                    }
                }
            }
        }
    }
}
"#;

/// Checks if OpenCL is available on the system
pub fn check_opencl_availability() -> bool {
    // If OpenCL support isn't compiled in, it's definitely not available
    #[cfg(not(feature = "opencl"))]
    return false;

    #[cfg(feature = "opencl")]
    {
        // If we've already checked, return the cached result
        let current = OPENCL_AVAILABLE.load(Ordering::Relaxed);
        if current {
            return true;
        }

        // Try to check for OpenCL availability
        // First by using the opencl3 library
        match opencl3::platform::get_platforms() {
            Ok(platforms) => {
                if !platforms.is_empty() {
                    // Found at least one platform
                    OPENCL_AVAILABLE.store(true, Ordering::Relaxed);
                    return true;
                }
            }
            Err(_) => {
                // Failed to get platforms, try dynamic loading
            }
        }

        // If opencl3 failed, try dynamic loading as a fallback
        use libloading::{Library, Symbol};
        
        let lib_names = if cfg!(target_os = "windows") {
            vec!["OpenCL.dll"]
        } else if cfg!(target_os = "macos") {
            vec!["libOpenCL.dylib", "/System/Library/Frameworks/OpenCL.framework/OpenCL"]
        } else {
            vec!["libOpenCL.so", "libOpenCL.so.1"]
        };
        
        for lib_name in lib_names {
            if let Ok(_) = unsafe { Library::new(lib_name) } {
                // Successfully loaded the library
                OPENCL_AVAILABLE.store(true, Ordering::Relaxed);
                return true;
            }
        }

        // If we get here, OpenCL is not available
        false
    }
}

/// GPU-based private key searcher
pub struct GpuSearcher {
    #[cfg(feature = "opencl")]
    device_count: usize,
    
    #[cfg(feature = "opencl")]
    selected_device_index: Option<usize>,
    
    #[cfg(feature = "opencl")]
    context: Option<Context>,
    
    #[cfg(feature = "opencl")]
    queue: Option<CommandQueue>,
    
    #[cfg(feature = "opencl")]
    program: Option<Program>,
}

impl GpuSearcher {
    /// Creates a new GPU searcher if OpenCL is available
    pub fn new() -> Result<Self, Box<dyn Error>> {
        // Check if OpenCL is available
        if !check_opencl_availability() {
            return Err("OpenCL is not available on this system".into());
        }
        
        #[cfg(feature = "opencl")]
        {
            let device_count = Self::count_devices()?;
            if device_count == 0 {
                return Err("No OpenCL devices found".into());
            }
            
            Ok(GpuSearcher {
                device_count,
                selected_device_index: None,
                context: None,
                queue: None,
                program: None,
            })
        }
        
        #[cfg(not(feature = "opencl"))]
        Err("OpenCL support not compiled in this build".into())
    }
    
    /// Count available OpenCL devices
    #[cfg(feature = "opencl")]
    fn count_devices() -> Result<usize, Box<dyn Error>> {
        let mut device_count = 0;
        
        // Get platforms
        let platforms = opencl3::platform::get_platforms()
            .map_err(|e| format!("Failed to get OpenCL platforms: {}", e))?;
        
        // Count devices on all platforms
        for platform in platforms {
            if let Ok(devices) = opencl3::device::get_device_ids(
                platform.id(), CL_DEVICE_TYPE_ALL) {
                device_count += devices.len();
            }
        }
        
        Ok(device_count)
    }
    
    /// List all available OpenCL devices
    pub fn list_devices(&self) -> Vec<(String, Device)> {
        #[cfg(feature = "opencl")]
        {
            let mut devices = Vec::new();
            
            // Get platforms
            match opencl3::platform::get_platforms() {
                Ok(platforms) => {
                    for platform in platforms {
                        // Get devices from platform
                        if let Ok(device_ids) = opencl3::device::get_device_ids(
                            platform.id(), CL_DEVICE_TYPE_ALL) {
                            for device_id in device_ids {
                                // Create device and get name
                                let device = Device::new(device_id);
                                let name = device.name().unwrap_or_else(|_| 
                                    "Unknown Device".to_string());
                                
                                devices.push((name, device));
                            }
                        }
                    }
                }
                Err(_) => {}
            }
            
            devices
        }
        
        #[cfg(not(feature = "opencl"))]
        Vec::new()
    }
    
    /// Select a specific device to use for computation
    pub fn select_device(&mut self, device_idx: usize) -> Result<(), Box<dyn Error>> {
        #[cfg(feature = "opencl")]
        {
            let devices = self.list_devices();
            if device_idx >= devices.len() {
                return Err("Invalid device index".into());
            }
            
            // Store the device index
            self.selected_device_index = Some(device_idx);
            
            // Create context and command queue
            let device_id = devices[device_idx].1.id();

            // Create a new Device with the same ID
            let device = Device::new(device_id);

            // Create context with this device
            let context = Context::from_device(device)
                .map_err(|e| format!("Failed to create OpenCL context: {}", e))?;
            
            let queue = CommandQueue::create(
                    context.get(),
                    device_id,
                    CL_QUEUE_PROFILING_ENABLE)
                .map_err(|e| format!("Failed to create command queue: {}", e))?;
            
            self.context = Some(context);
            self.queue = Some(queue);
            
            Ok(())
        }
        
        #[cfg(not(feature = "opencl"))]
        Err("OpenCL support not compiled in this build".into())
    }
    
    /// Initialize the OpenCL program with the kernel code
    pub fn initialize_program(&mut self) -> Result<(), Box<dyn Error>> {
        #[cfg(feature = "opencl")]
        {
            if self.context.is_none() || self.queue.is_none() {
                return Err("Device not selected".into());
            }

            // Load the OpenCL kernel (will use embedded kernel if file loading fails)
            let kernel_src = self.load_kernel_source()?;
            
            // Convert kernel source to C string
            use std::ffi::CString;
            let c_src = CString::new(kernel_src).map_err(|e| 
                format!("Failed to convert kernel to CString: {}", e))?;
            
            // Create program from source
            let context = self.context.as_ref().unwrap();
            let devices = self.list_devices();
            let device_idx = self.selected_device_index.unwrap();
            let device = &devices[device_idx].1;
            
            let program = Program::create_from_source(
                    context.get(),
                    &[c_src.as_c_str()])
                .map_err(|e| format!("Failed to create OpenCL program: {}", e))?;
            
            // Build the program
            let empty_options = std::ffi::CString::new("").unwrap();
            program.build(&[device.id()], &empty_options)
                .map_err(|e| format!("Failed to build OpenCL program: {}", e))?;
            
            self.program = Some(program);
            
            Ok(())
        }
        
        #[cfg(not(feature = "opencl"))]
        Err("OpenCL support not compiled in this build".into())
    }
    
    /// Load the OpenCL kernel source from file
    fn load_kernel_source(&self) -> Result<String, Box<dyn Error>> {
        // First try loading from the build directory
        match env::var("OUT_DIR") {
            Ok(out_dir) => {
                let kernel_path = Path::new(&out_dir).join("crypto_kernels.cl");
                if kernel_path.exists() {
                    match fs::read_to_string(kernel_path) {
                        Ok(content) => return Ok(content),
                        Err(_) => {} // Continue to next attempt
                    }
                }
            },
            Err(_) => {} // Continue to next attempt if OUT_DIR isn't set
        }
        
        // Next try loading from the source directory
        let kernel_path = Path::new("src").join("crypto_kernels.cl");
        if kernel_path.exists() {
            match fs::read_to_string(kernel_path) {
                Ok(content) => return Ok(content),
                Err(_) => {} // Continue to embedded kernel
            }
        }
        
        // If we get here, use the embedded kernel without showing an error
        println!("Using embedded OpenCL kernel");
        Ok(KERNEL_SRC.to_string())
    }

    /// OpenCL search implementation
    pub fn search(&self, targets: &HashSet<[u8; 20]>, range_start: u64, range_end: u64, 
                batch_size: usize) -> Result<Vec<u64>, Box<dyn Error>> {
        #[cfg(feature = "opencl")]
        {
            if self.context.is_none() || self.queue.is_none() || self.program.is_none() {
                return Err("OpenCL context not initialized. Call initialize_program() first.".into());
            }
            
            // Define the target data
            let target_count = targets.len();
            let mut flattened_targets = Vec::with_capacity(target_count * 20);
            for target in targets {
                flattened_targets.extend_from_slice(target);
            }
            
            // Check if the range is too large for u64
            let range_size = if range_end > range_start {
                range_end - range_start
            } else {
                return Ok(vec![]); // Empty range
            };
            
            // Calculate maximum batch size that GPU can handle
            // For extremely large ranges, we'll use multiple smaller batches
            let max_gpu_batch = 1_000_000; // Limit batch size to avoid overflowing GPU
            let work_size = std::cmp::min(batch_size, max_gpu_batch);
            
            // Process data in smaller chunks the GPU can handle
            let mut all_found_keys = Vec::new();
            
            // Calculate step size for processing the range
            let total_steps = (range_size as f64 / max_gpu_batch as f64).ceil() as u64;
            let step_size = range_size / total_steps;
            
            println!("Dividindo intervalo em {} pedaços menores para GPU", total_steps);
            
            // Process each chunk
            for step in 0..total_steps {
                let chunk_start = range_start + (step * step_size);
                let chunk_end = if step == total_steps - 1 {
                    range_end
                } else {
                    chunk_start + step_size
                };
                
                // Skip empty ranges
                if chunk_end <= chunk_start {
                    continue;
                }
                
                println!("Processando chunk GPU {}/{}: [{} - {}]", 
                         step+1, total_steps, chunk_start, chunk_end);
                
                // Set up work distribution for this chunk
                let chunk_size = chunk_end - chunk_start;
                let items_per_work = (chunk_size as f64 / work_size as f64).ceil() as u64;
                
                if items_per_work == 0 {
                    continue;
                }
                
                // Create key ranges for this chunk
                let mut key_ranges = Vec::with_capacity(work_size * 2);
                
                for i in 0..work_size {
                    let start = chunk_start + (i as u64 * items_per_work);
                    let end = std::cmp::min(
                        chunk_start + ((i as u64 + 1) * items_per_work),
                        chunk_end
                    );
                    
                    // Skip if start >= end
                    if start >= end {
                        continue;
                    }
                    
                    key_ranges.push(start);
                    key_ranges.push(end);
                }
                
                // Skip if no valid ranges
                if key_ranges.is_empty() {
                    continue;
                }
                
                // Prepare buffers for OpenCL
                let context = self.context.as_ref().unwrap();
                let queue = self.queue.as_ref().unwrap();
                let program = self.program.as_ref().unwrap();
                
                // Create buffers
                let max_results = 100;
                let mut results = vec![0u32; 1 + max_results];
                let mut found_keys = vec![0u64; max_results];
                let num_targets = vec![target_count as u32];
                
                // Create OpenCL buffers
                let key_ranges_buf = Buffer::create(
                    context,
                    CL_MEM_READ_ONLY,
                    std::mem::size_of::<u64>() * key_ranges.len(),
                    std::ptr::null_mut()
                ).map_err(|e| format!("Failed to create key ranges buffer: {}", e))?;
                
                let targets_buf = Buffer::create(
                    context,
                    CL_MEM_READ_ONLY,
                    flattened_targets.len(),
                    std::ptr::null_mut()
                ).map_err(|e| format!("Failed to create targets buffer: {}", e))?;
                
                let num_targets_buf = Buffer::create(
                    context,
                    CL_MEM_READ_ONLY,
                    std::mem::size_of::<u32>(),
                    std::ptr::null_mut()
                ).map_err(|e| format!("Failed to create num targets buffer: {}", e))?;
                
                let results_buf = Buffer::create(
                    context,
                    CL_MEM_WRITE_ONLY,
                    std::mem::size_of::<u32>() * results.len(),
                    std::ptr::null_mut()
                ).map_err(|e| format!("Failed to create results buffer: {}", e))?;
                
                let found_keys_buf = Buffer::create(
                    context,
                    CL_MEM_WRITE_ONLY,
                    std::mem::size_of::<u64>() * found_keys.len(),
                    std::ptr::null_mut()
                ).map_err(|e| format!("Failed to create found keys buffer: {}", e))?;
                
                // Write data to buffers
                queue.enqueue_write_buffer(&key_ranges_buf, CL_TRUE, 0, 
                    &key_ranges, &[]).map_err(|e| format!("Failed to write key_ranges: {}", e))?;
                
                queue.enqueue_write_buffer(&targets_buf, CL_TRUE, 0, 
                    &flattened_targets, &[]).map_err(|e| format!("Failed to write targets: {}", e))?;
                
                queue.enqueue_write_buffer(&num_targets_buf, CL_TRUE, 0, 
                    &num_targets, &[]).map_err(|e| format!("Failed to write num_targets: {}", e))?;
                
                queue.enqueue_write_buffer(&results_buf, CL_TRUE, 0, 
                    &results, &[]).map_err(|e| format!("Failed to write results: {}", e))?;
                
                queue.enqueue_write_buffer(&found_keys_buf, CL_TRUE, 0, 
                    &found_keys, &[]).map_err(|e| format!("Failed to write found_keys: {}", e))?;
                
                // Create kernel
                use std::ffi::CString;
                let kernel_name = CString::new("search_keys").unwrap();
                
                // Create the kernel and set arguments
                let kernel = unsafe {
                    // Get the raw program pointer using transmute (this is unsafe but necessary)
                    let program_ptr = program as *const Program as *mut std::ffi::c_void;
                    
                    // Create kernel using opencl3 API with the raw pointer
                    let kernel_ptr = opencl3::kernel::create_kernel(
                        program_ptr, kernel_name.as_c_str())
                        .map_err(|e| format!("Failed to create kernel: {}", e))?;
                    
                    Kernel::new(kernel_ptr)
                        .map_err(|e| format!("Failed to create kernel object: {}", e))?
                };
                
                // Set kernel arguments
                kernel.set_arg(0, &key_ranges_buf)
                    .map_err(|e| format!("Failed to set kernel arg 0: {}", e))?;
                
                kernel.set_arg(1, &targets_buf)
                    .map_err(|e| format!("Failed to set kernel arg 1: {}", e))?;
                
                kernel.set_arg(2, &num_targets_buf)
                    .map_err(|e| format!("Failed to set kernel arg 2: {}", e))?;
                
                kernel.set_arg(3, &results_buf)
                    .map_err(|e| format!("Failed to set kernel arg 3: {}", e))?;
                
                kernel.set_arg(4, &found_keys_buf)
                    .map_err(|e| format!("Failed to set kernel arg 4: {}", e))?;
                
                // Execute kernel
                let work_items = key_ranges.len() / 2; // Number of actual work items
                let global_work_size = [work_items];
                
                opencl3::command_queue::enqueue_nd_range_kernel(
                    queue.get(),
                    kernel.get(),
                    1, // work_dim
                    std::ptr::null(),
                    global_work_size.as_ptr(),
                    std::ptr::null(),
                    0,
                    std::ptr::null()
                ).map_err(|e| format!("Failed to enqueue kernel: {}", e))?;
                
                // Read results
                queue.enqueue_read_buffer(&results_buf, CL_TRUE, 0, &mut results, &[])
                    .map_err(|e| format!("Failed to read results buffer: {}", e))?;
                
                queue.enqueue_read_buffer(&found_keys_buf, CL_TRUE, 0, &mut found_keys, &[])
                    .map_err(|e| format!("Failed to read found keys buffer: {}", e))?;
                
                // Process found keys for this chunk
                let num_found = results[0] as usize;
                if num_found > 0 {
                    all_found_keys.extend_from_slice(&found_keys[0..num_found.min(max_results)]);
                }
            }
            
            Ok(all_found_keys)
        }
        
        #[cfg(not(feature = "opencl"))]
        Err("OpenCL support not compiled in this build".into())
    }
} 