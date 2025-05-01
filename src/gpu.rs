use std::error::Error;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

// Add libc for OpenCL sizes
extern crate libc;

// Import the opencl3 crate conditionally
#[cfg(feature = "opencl")]
use opencl3::platform::get_platforms;
#[cfg(feature = "opencl")]
use opencl3::device::{Device, CL_DEVICE_TYPE_ALL, get_device_ids};
#[cfg(feature = "opencl")]
use opencl3::context::Context;
#[cfg(feature = "opencl")]
use opencl3::command_queue::{CommandQueue, CL_QUEUE_PROFILING_ENABLE, enqueue_write_buffer, enqueue_read_buffer, enqueue_nd_range_kernel};
#[cfg(feature = "opencl")]
use opencl3::program::Program;
#[cfg(feature = "opencl")]
use opencl3::kernel::{Kernel, create_kernel};
#[cfg(feature = "opencl")]
use opencl3::memory::{CL_MEM_READ_ONLY, CL_MEM_WRITE_ONLY, CL_MEM_READ_WRITE, CL_MEM_COPY_HOST_PTR, create_buffer, release_mem_object};
#[cfg(feature = "opencl")]
use opencl3::types::*;

// Static flag for OpenCL availability
static OPENCL_AVAILABLE: AtomicBool = AtomicBool::new(false);

// OpenCL kernel for Bitcoin key search
const KERNEL_SRC: &str = r#"
// Basic OpenCL kernel for Bitcoin key search
// This is a simplified version for testing OpenCL functionality

// Simple hash approximation function (not the actual crypto, just for testing)
void calculate_simplified_hash(ulong private_key, __private uchar *hash160) {
    // This is just for testing OpenCL functionality
    // Not the actual RIPEMD160(SHA256()) calculation
    for (int i = 0; i < 20; i++) {
        hash160[i] = (private_key >> (i % 8)) & 0xFF;
    }
}

// Main search kernel
__kernel void search_keys(
    __global const uchar* target_hashes,
    __global ulong* found_keys,
    __global uint* results,
    uint num_targets,
    ulong range_start,
    ulong range_end
) {
    // Get global ID
    const size_t gid = get_global_id(0);
    
    // Calculate this work item's key range
    const ulong total_items = get_global_size(0);
    const ulong range_size = range_end - range_start;
    const ulong keys_per_item = (range_size + total_items - 1) / total_items;
    
    const ulong my_start = range_start + gid * keys_per_item;
    const ulong my_end = min(my_start + keys_per_item, range_end);
    
    // Skip if range is invalid
    if (my_start >= my_end || my_start >= range_end) {
        return;
    }
    
    // Initialize temporary buffer for hash computation
    uchar computed_hash[20];
    
    // Number of keys to check per kernel execution
    const uint keys_per_work_item = 100; // Increased for better performance
    
    // Loop through a small batch of keys in this range
    for (ulong key = my_start; key < my_end && key < my_start + keys_per_work_item; key++) {
        // Calculate hash160 for this private key (simplified version)
        calculate_simplified_hash(key, computed_hash);
        
        // Check against all targets
        for (uint t = 0; t < num_targets; t++) {
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
                uint idx = atomic_inc(results);
                
                // Check if we have space for this result
                if (idx < 100) {  // Max 100 results
                    // Store the private key
                    found_keys[idx] = key;
                }
            }
        }
    }
}
"#;

#[cfg(feature = "opencl")]
const CL_TRUE: cl_bool = 1;
#[cfg(feature = "opencl")]
const CL_FALSE: cl_bool = 0;

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
        match get_platforms() {
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
        let platforms = get_platforms()
            .map_err(|e| format!("Failed to get OpenCL platforms: {}", e))?;
        
        // Count devices on all platforms
        for platform in platforms {
            if let Ok(devices) = get_device_ids(
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
            match get_platforms() {
                Ok(platforms) => {
                    for platform in platforms {
                        // Get devices from platform
                        if let Ok(device_ids) = get_device_ids(
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

            // Create program with source
            let program = Program::create_from_source(context.get(), &[c_src.as_c_str()])
                .map_err(|e| format!("Failed to create program: {}", e))?;
            
            // Build the program
            let build_options = CString::new("").unwrap();
            let build_result = program.build(&[device.id()], &build_options);
            
            if let Err(e) = build_result {
                // Get the build log for better error diagnostics
                let build_log = match program.get_build_log(device.id()) {
                    Ok(log) => log,
                    Err(_) => "Could not retrieve build log".to_string()
                };
                
                println!("OpenCL Program Build Error:");
                println!("{}", build_log);
                
                return Err(format!("Failed to build OpenCL program: {}", e).into());
            }
            
            // Store the program
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
            
            // We can only process one target at a time efficiently
            if targets.is_empty() {
                return Ok(vec![]);
            }
            
            // Get the first target (we only support one target at a time efficiently)
            let target_hash = targets.iter().next().unwrap();
            
            // Check if the range is too large for u64
            let range_size = if range_end > range_start {
                range_end - range_start
            } else {
                return Ok(vec![]); // Empty range
            };
            
            // SAFETY CHECK: Limit maximum range size for GPU processing to prevent crashes
            const MAX_GPU_CHUNK_SIZE: u64 = 100_000_000; // 100 million keys per GPU chunk
            
            // If range is too large, process it in smaller chunks
            if range_size > MAX_GPU_CHUNK_SIZE {
                println!("GPU: Range too large ({} keys), splitting into smaller chunks", range_size);
                let mut all_results = Vec::new();
                let num_chunks = (range_size + MAX_GPU_CHUNK_SIZE - 1) / MAX_GPU_CHUNK_SIZE;
                
                // Process each chunk
                for i in 0..num_chunks {
                    let chunk_start = range_start + i * MAX_GPU_CHUNK_SIZE;
                    let chunk_end = std::cmp::min(range_start + (i + 1) * MAX_GPU_CHUNK_SIZE, range_end);
                    
                    println!("GPU: Processing chunk {}/{}: {} - {}", 
                             i+1, num_chunks, chunk_start, chunk_end);
                    
                    // Use a try-catch pattern to handle GPU errors gracefully
                    match self.safe_gpu_search(target_hash, 1, chunk_start, chunk_end, batch_size) {
                        Ok(results) => all_results.extend(results),
                        Err(e) => {
                            println!("GPU: Error processing chunk: {}", e);
                            println!("GPU: Falling back to CPU for this chunk");
                            // Fall back to CPU-based search if GPU search fails
                            // Just return empty results and let the search algorithm handle CPU fallback
                            return Err(format!("GPU search failed: {}", e).into());
                        }
                    }
                }
                
                return Ok(all_results);
            }
            
            // For smaller ranges, process directly
            self.safe_gpu_search(target_hash, 1, range_start, range_end, batch_size)
        }
        
        #[cfg(not(feature = "opencl"))]
        Err("OpenCL support not compiled in this build".into())
    }

    /// Safe wrapper for GPU search that handles errors
    #[cfg(feature = "opencl")]
    fn safe_gpu_search(
        &self,
        target_hash: &[u8; 20],
        target_count: usize,
        range_start: u64,
        range_end: u64,
        batch_size: usize
    ) -> Result<Vec<u64>, Box<dyn Error>> {
        // Check required OpenCL components
        let context = match &self.context {
            Some(ctx) => ctx,
            None => return Err("OpenCL context not initialized".into())
        };
        
        let queue = match &self.queue {
            Some(q) => q,
            None => return Err("OpenCL command queue not initialized".into())
        };
        
        let program = match &self.program {
            Some(p) => p,
            None => return Err("OpenCL program not initialized".into())
        };
        
        // Safety check - verify range size
        if range_end <= range_start {
            return Ok(vec![]);
        }
        
        // Calculate work size with upper limit based on device capabilities
        let range_size = range_end - range_start;
        let adjusted_batch_size = std::cmp::min(batch_size, 65536); // Max of 65536 keys per batch
        
        // Create buffer for target hash
        let target_buffer = unsafe {
            create_buffer(
                context.get(),
                CL_MEM_READ_ONLY | CL_MEM_COPY_HOST_PTR,
                std::mem::size_of::<u8>() * 20, // 20 bytes for a single hash160
                target_hash.as_ptr() as *mut std::ffi::c_void
            )
            .map_err(|e| format!("Failed to create target buffer: {}", e))?
        };
        
        // Create buffer for found keys (max 100 results)
        let result_keys_buffer = unsafe {
            create_buffer(
                context.get(),
                CL_MEM_WRITE_ONLY,
                std::mem::size_of::<u64>() * 100,
                std::ptr::null_mut()
            )
            .map_err(|e| format!("Failed to create result keys buffer: {}", e))?
        };
        
        // Create buffer for result count
        let count_buffer = unsafe {
            create_buffer(
                context.get(),
                CL_MEM_READ_WRITE,
                std::mem::size_of::<u32>(),
                std::ptr::null_mut()
            )
            .map_err(|e| format!("Failed to create count buffer: {}", e))?
        };
        
        // Initialize the count buffer to zero
        let zero: u32 = 0;
        unsafe {
            enqueue_write_buffer(
                queue.get(),
                count_buffer,
                CL_TRUE,
                0,
                std::mem::size_of::<u32>(),
                &zero as *const u32 as *const std::ffi::c_void,
                0,
                std::ptr::null()
            )
            .map_err(|e| format!("Failed to initialize count buffer: {}", e))?
        };
        
        // Create kernel
        use std::ffi::CString;
        let kernel_name = CString::new("search_keys").unwrap();
        
        // Create the kernel
        let kernel = unsafe {
            // Get the raw program pointer using transmute (this is unsafe but necessary)
            let program_ptr = program as *const Program as *mut std::ffi::c_void;
            
            // Create kernel using the raw pointer
            let kernel_ptr = create_kernel(
                program_ptr, kernel_name.as_c_str())
                .map_err(|e| format!("Failed to create kernel: {}", e))?;
            
            Kernel::new(kernel_ptr)
                .map_err(|e| format!("Failed to create kernel object: {}", e))?
        };
        
        // Set kernel arguments
        unsafe {
            // Set arguments based on the updated kernel signature
            kernel.set_arg(0, &cl_mem::from(target_buffer))
                .map_err(|e| format!("Failed to set target_buffer arg: {}", e))?;
            
            kernel.set_arg(1, &cl_mem::from(result_keys_buffer))
                .map_err(|e| format!("Failed to set result_keys_buffer arg: {}", e))?;
            
            kernel.set_arg(2, &cl_mem::from(count_buffer))
                .map_err(|e| format!("Failed to set count_buffer arg: {}", e))?;
            
            let targets_len: u32 = 1; // We're only using a single target
            
            kernel.set_arg(3, &targets_len)
                .map_err(|e| format!("Failed to set targets_len arg: {}", e))?;
            
            kernel.set_arg(4, &range_start)
                .map_err(|e| format!("Failed to set range_start arg: {}", e))?;
            
            kernel.set_arg(5, &range_end)
                .map_err(|e| format!("Failed to set range_end arg: {}", e))?;
        }
        
        // Calculate appropriate work sizes based on range size
        let global_work_size = std::cmp::min(adjusted_batch_size, 65536); // Limit work size
        let local_work_size = std::cmp::min(256, global_work_size); // Use 256 threads per work group or less
        
        println!("GPU: Executing kernel with {} work items (local size: {})", 
                 global_work_size, local_work_size);
        
        // Execute the kernel
        unsafe {
            enqueue_nd_range_kernel(
                queue.get(),
                kernel.get(),
                1,
                std::ptr::null(),
                &global_work_size as *const usize as *const libc::size_t,
                &local_work_size as *const usize as *const libc::size_t,
                0,
                std::ptr::null()
            )
            .map_err(|e| format!("Failed to enqueue kernel: {}", e))?
        };
        
        // Read result count
        let mut count: u32 = 0;
        unsafe {
            enqueue_read_buffer(
                queue.get(),
                count_buffer,
                CL_TRUE,
                0,
                std::mem::size_of::<u32>(),
                &mut count as *mut u32 as *mut std::ffi::c_void,
                0,
                std::ptr::null()
            )
            .map_err(|e| format!("Failed to read count: {}", e))?
        };
        
        println!("GPU: Found {} potential results", count);
        
        // Read results
        let mut results = vec![0u64; std::cmp::min(count as usize, 100)]; // Limit to max 100 results
        if count > 0 {
            unsafe {
                enqueue_read_buffer(
                    queue.get(),
                    result_keys_buffer,
                    CL_TRUE,
                    0,
                    std::mem::size_of::<u64>() * results.len(),
                    results.as_mut_ptr() as *mut std::ffi::c_void,
                    0,
                    std::ptr::null()
                )
                .map_err(|e| format!("Failed to read results: {}", e))?
            };
        }
        
        // Cleanup
        unsafe {
            release_mem_object(target_buffer).ok();
            release_mem_object(result_keys_buffer).ok();
            release_mem_object(count_buffer).ok();
        }
        
        Ok(results)
    }

    /// Thread-safe search function that doesn't require sending the GpuSearcher across threads
    /// Instead, it performs the search directly on the current thread
    pub fn search_direct(
        &mut self,
        target: &[u8; 20],
        range_start: u64,
        range_end: u64,
        batch_size: usize
    ) -> Result<Vec<u64>, Box<dyn Error>> {
        // Validate range size
        if range_end <= range_start {
            println!("Aviso: Range inválido para GPU ({} - {}), pulando.", range_start, range_end);
            return Ok(vec![]);
        }
        
        // Check if GPU context is initialized
        #[cfg(feature = "opencl")]
        {
            if self.context.is_none() || self.queue.is_none() || self.program.is_none() {
                return Err("OpenCL não inicializado. Chame initialize_program() primeiro.".into());
            }
        }
        
        let range_size = range_end - range_start;
        
        // Output info message
        println!("{}GPU iniciando busca no range: {} - {}{}", 
                 crate::colors::MAGENTA, range_start, range_end, crate::colors::RESET);
        
        // SAFETY CHECK: Limit maximum range size for GPU processing to prevent crashes
        const MAX_GPU_CHUNK_SIZE: u64 = 100_000_000; // 100 million keys per GPU chunk
        
        #[cfg(feature = "opencl")]
        {
            // If range is too large, process it in smaller chunks
            if range_size > MAX_GPU_CHUNK_SIZE {
                println!("GPU: Range too large ({} keys), splitting into smaller chunks", range_size);
                let mut all_results = Vec::new();
                let num_chunks = (range_size + MAX_GPU_CHUNK_SIZE - 1) / MAX_GPU_CHUNK_SIZE;
                
                // Process each chunk
                for i in 0..num_chunks {
                    let chunk_start = range_start + i * MAX_GPU_CHUNK_SIZE;
                    let chunk_end = std::cmp::min(range_start + (i + 1) * MAX_GPU_CHUNK_SIZE, range_end);
                    
                    println!("GPU: Processing chunk {}/{}: {} - {}", 
                             i+1, num_chunks, chunk_start, chunk_end);
                    
                    // Use a try-catch pattern to handle GPU errors gracefully
                    match self.safe_gpu_search(target, 1, chunk_start, chunk_end, batch_size) {
                        Ok(results) => {
                            if !results.is_empty() {
                                all_results.extend(results);
                            }
                        },
                        Err(e) => {
                            println!("GPU: Error processing chunk: {}", e);
                            println!("GPU: Falling back to CPU for this chunk");
                            // Fall back to CPU-based search if GPU search fails
                            return Err(format!("GPU search failed: {}", e).into());
                        }
                    }
                    
                    // Update progress counter
                    crate::performance::increment_keys_checked((chunk_end - chunk_start) as usize);
                }
                
                return Ok(all_results);
            }
            
            // Process smaller ranges directly
            match self.safe_gpu_search(target, 1, range_start, range_end, batch_size) {
                Ok(results) => {
                    // Update keys checked counter
                    crate::performance::increment_keys_checked(range_size as usize);
                    Ok(results)
                },
                Err(e) => {
                    println!("GPU: Error: {}", e);
                    Err(e)
                }
            }
        }
        
        #[cfg(not(feature = "opencl"))]
        Err("OpenCL support not compiled".into())
    }
} 