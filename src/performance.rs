use std::sync::atomic::{AtomicUsize, Ordering};
use sysinfo::{System, SystemExt, CpuExt};
use num_bigint::BigUint;
use num_traits::{ToPrimitive, Zero};
use rayon::ThreadPoolBuilder;
use std::default::Default;
use num_cpus;

// Detecção de instruções SIMD
#[cfg(target_arch = "x86")]
use std::arch::x86::*;
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::*;

// Estrutura para armazenar informações sobre recursos do sistema
pub struct SystemResources {
    pub total_memory: u64,        // Total de memória em bytes
    pub available_memory: u64,    // Memória disponível em bytes
    pub cpu_count: usize,         // Número de núcleos físicos
    pub thread_count: usize,      // Número de threads lógicos
    #[allow(dead_code)]
    pub cpu_usage: f32,           // Uso atual da CPU (0-100%)
    pub cpu_brand: String,        // Informação da CPU (marca/modelo)
    pub has_avx: bool,            // Suporte a AVX
    pub has_avx2: bool,           // Suporte a AVX2
    pub has_sse: bool,            // Suporte a SSE
}

// Parâmetros otimizados para busca
pub struct SearchParameters {
    pub threads: usize,          // Número de threads a usar
    pub batch_size: usize,        // Tamanho do batch para processamento
    pub resource_usage: u8,       // Percentual de recursos a utilizar (1-100)
    
    // Essas opções são usadas internamente pelo sistema
    #[allow(dead_code)]
    pub memory_limit: usize,      // Limite de memória em bytes
    #[allow(dead_code)]
    pub use_simd: bool,           // Se deve usar instruções SIMD
}

impl Default for SearchParameters {
    fn default() -> Self {
        SearchParameters {
            threads: num_cpus::get(),
            batch_size: 4096,
            resource_usage: 75,
            memory_limit: 1024 * 1024 * 1024, // 1GB default
            use_simd: true,
        }
    }
}

// Detecta recursos do sistema
pub fn detect_system_resources() -> SystemResources {
    let mut sys = System::new_all();
    sys.refresh_all();
    
    // Informações de CPU
    let cpu_brand = if !sys.cpus().is_empty() {
        sys.cpus()[0].brand().to_string()
    } else {
        "Unknown".to_string()
    };
    
    // Get CPU info and determine which SIMD instructions are available
    let (has_avx, has_avx2, has_sse) = {
        (is_x86_feature_detected!("avx"),
         is_x86_feature_detected!("avx2"),
         is_x86_feature_detected!("sse4.1"))
    };
    
    let total_memory = sys.total_memory();
    let available_memory = sys.available_memory();
    let cpu_physical_count = num_cpus::get_physical();
    let cpu_logical_count = num_cpus::get();
    
    // Uso médio da CPU
    let mut cpu_usage_total = 0.0;
    let cpu_count = sys.cpus().len();
    for cpu in sys.cpus() {
        cpu_usage_total += cpu.cpu_usage();
    }
    let cpu_usage = if cpu_count > 0 {
        cpu_usage_total / cpu_count as f32
    } else {
        0.0
    };
    
    SystemResources {
        total_memory,
        available_memory,
        cpu_count: cpu_physical_count,
        thread_count: cpu_logical_count,
        cpu_usage,
        cpu_brand,
        has_avx,
        has_avx2,
        has_sse,
    }
}

// Calcula parâmetros otimizados para a busca
pub fn calculate_optimal_parameters(resources: &SystemResources, usage_percentage: u8) -> SearchParameters {
    // Limitar o percentual entre 10% e 100%
    let usage_percentage = usage_percentage.clamp(10, 100);
    
    // Calcular número de threads com base no percentual solicitado
    let thread_count = {
        let requested_threads = (resources.thread_count as f32 * usage_percentage as f32 / 100.0).ceil() as usize;
        requested_threads.max(1) // No mínimo 1 thread
    };
    
    // Tamanho do batch otimizado com base no tipo de CPU e memória disponível
    let batch_size = if resources.available_memory > 8 * 1024 * 1024 * 1024 {
        // Para máquinas com mais de 8GB disponível
        8192
    } else if resources.available_memory > 4 * 1024 * 1024 * 1024 {
        // Para máquinas com mais de 4GB disponível
        4096
    } else {
        // Para máquinas com memória limitada
        2048
    };
    
    // Memória a reservar para a busca
    let memory_limit = {
        let memory_percentage = (usage_percentage as f64 * 0.8) as f64; // Usar 80% do percentual solicitado para memória
        (resources.available_memory as f64 * memory_percentage / 100.0) as usize
    };
    
    // Decidir se usa SIMD conforme recursos disponíveis
    let use_simd = resources.has_avx2 || resources.has_avx || resources.has_sse;
    
    SearchParameters {
        threads: thread_count,
        batch_size,
        memory_limit,
        use_simd,
        resource_usage: usage_percentage,
    }
}

// Configura o thread pool global com os parâmetros otimizados
pub fn configure_thread_pool(params: &SearchParameters) -> Result<(), String> {
    match ThreadPoolBuilder::new()
        .num_threads(params.threads)
        .build_global() {
        Ok(_) => Ok(()),
        Err(e) => {
            // Verificar se o erro é GlobalPoolAlreadyInitialized
            if format!("{:?}", e).contains("GlobalPoolAlreadyInitialized") {
                // O pool já está inicializado, isso não é um erro crítico
                // Apenas informe e retorne OK para continuar
                println!("Nota: Pool de threads global já inicializado. Continuando com a configuração existente.");
                Ok(())
            } else {
                // Outros erros são reportados normalmente
                Err(format!("Erro ao configurar thread pool: {}", e))
            }
        }
    }
}

// Estima a velocidade de busca com base nos recursos disponíveis
pub fn estimate_search_speed(resources: &SystemResources, params: &SearchParameters) -> u64 {
    let base_keys_per_thread = 750_000; // Estimativa base por thread/segundo
    
    // Ajuste por tipo de CPU
    let cpu_multiplier = if resources.has_avx2 {
        1.5 // CPUs com AVX2 são mais rápidas
    } else if resources.has_avx {
        1.3 // CPUs com AVX são um pouco mais rápidas
    } else {
        1.0 // Base para CPUs sem extensões específicas
    };
    
    // Ajuste de velocidade com base na memória disponível
    let memory_multiplier = if resources.available_memory > 8 * 1024 * 1024 * 1024 {
        1.2 // Mais memória = menos swapping = maior velocidade
    } else if resources.available_memory > 4 * 1024 * 1024 * 1024 {
        1.0 // Base para 4-8GB
    } else {
        0.8 // Menos memória = possível swapping = menor velocidade
    };
    
    // Velocidade estimada por segundo (ajustada pelo uso de recursos)
    (base_keys_per_thread as f64 
        * params.threads as f64 
        * cpu_multiplier 
        * memory_multiplier 
        * (params.resource_usage as f64 / 100.0)) as u64
}

// Estima o tempo necessário para busca em um intervalo
#[allow(dead_code)]
pub fn estimate_search_time(min_key: &BigUint, max_key: &BigUint, keys_per_sec: u64) -> f64 {
    let range_size = (max_key - min_key).to_f64().unwrap_or(f64::MAX);
    range_size / keys_per_sec as f64
}

// Otimiza a distribuição de trabalho com base no intervalo e nos recursos
pub fn optimize_workload_distribution(
    min_key: &BigUint, 
    max_key: &BigUint, 
    params: &SearchParameters
) -> Vec<(BigUint, BigUint)> {
    let range_size = max_key - min_key;
    let chunk_count = params.threads * 4; // Usar 4x mais chunks que threads para balanceamento
    
    let mut chunks = Vec::with_capacity(chunk_count);
    let chunk_size = &range_size / chunk_count;
    
    if chunk_size.is_zero() {
        // Intervalo muito pequeno para dividir
        chunks.push((min_key.clone(), max_key.clone()));
        return chunks;
    }
    
    let mut current = min_key.clone();
    for _ in 0..chunk_count-1 {
        let next = &current + &chunk_size;
        chunks.push((current.clone(), next.clone()));
        current = next;
    }
    
    // O último chunk pode ser um pouco maior para compensar arredondamentos
    chunks.push((current, max_key.clone()));
    
    chunks
}

// Reset the keys checked counter (used between searches)
pub fn reset_keys_checked() {
    // Access the static counter in search.rs and reset it
    unsafe {
        let ptr = &crate::search::KEYS_CHECKED as *const std::sync::atomic::AtomicUsize;
        (*ptr).store(0, std::sync::atomic::Ordering::SeqCst);
    }
}

// Get the current number of keys checked
pub fn get_keys_checked() -> usize {
    // Access the static counter in search.rs
    unsafe {
        let ptr = &crate::search::KEYS_CHECKED as *const std::sync::atomic::AtomicUsize;
        (*ptr).load(std::sync::atomic::Ordering::Relaxed)
    }
}

// Increment the keys checked counter by the specified amount
pub fn increment_keys_checked(count: usize) {
    // Access the static counter in search.rs
    unsafe {
        let ptr = &crate::search::KEYS_CHECKED as *const std::sync::atomic::AtomicUsize;
        (*ptr).fetch_add(count, Ordering::Relaxed);
    }
} 