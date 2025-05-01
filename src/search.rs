use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::thread;

use num_bigint::BigUint;
use num_traits::{Zero, ToPrimitive};
use chrono::Local;
use rayon::prelude::*;
use hex;

use crate::bitcoin::{pad_private_key, private_key_to_hash160, private_key_to_wif, private_key_to_p2pkh_address, validate_private_key_for_hash160};
use crate::colors;
use crate::performance;

#[cfg(feature = "opencl")]
use crate::gpu;
#[cfg(feature = "opencl")]
use std::collections::HashSet;

#[cfg(feature = "opencl")]
type OptionalGpuSearcher = Option<gpu::GpuSearcher>;

#[cfg(not(feature = "opencl"))]
type OptionalGpuSearcher = Option<()>; // Using unit type when GPU support is not compiled

// Global counter for keys checked
pub static KEYS_CHECKED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

// Constants for display updates
const STATS_UPDATE_INTERVAL: u64 = 30; // Update display every 30 seconds

// Compare two byte slices for equality - versão otimizada
fn bytes_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    
    // Usando iteração e verificação byte-a-byte
    a.iter().zip(b.iter()).all(|(x, y)| x == y)
}

// Helper function to pad a private key to 32 bytes (internal implementation)
fn pad_private_key_internal(key: &BigUint) -> Vec<u8> {
    let mut bytes = key.to_bytes_be();
    
    // Ensure key is 32 bytes
    if bytes.len() < 32 {
        let padding = vec![0u8; 32 - bytes.len()];
        let mut padded = padding;
        padded.extend_from_slice(&bytes);
        bytes = padded;
    } else if bytes.len() > 32 {
        // Truncate if somehow longer than 32 bytes
        bytes = bytes[bytes.len() - 32..].to_vec();
    }
    
    bytes
}

// Função para procurar a chave privada dentro de um intervalo específico
#[allow(dead_code)]
pub fn search_for_private_key(min_key: &BigUint, max_key: &BigUint, target_hash160: &[u8]) {
    let range_size = max_key - min_key;
    let range_size_f64 = range_size.to_f64().unwrap_or(f64::MAX);
    let is_range_zero = range_size.is_zero();
    
    println!("{}Iniciando busca no intervalo especificado...{}", colors::BOLD_GREEN, colors::RESET);
    println!("{}Range: {} a {}{}", colors::CYAN, min_key.to_str_radix(16), max_key.to_str_radix(16), colors::RESET);
    
    let start_time = Instant::now();
    let keys_checked = Arc::new(AtomicU64::new(0));
    let found = Arc::new(AtomicBool::new(false));
    let found_key = Arc::new(Mutex::new(BigUint::zero()));
    
    // Thread para exibir estatísticas periódicas
    let keys_checked_clone = Arc::clone(&keys_checked);
    let found_clone = Arc::clone(&found);
    let range_size_f64_clone = range_size_f64;
    let stats_thread = thread::spawn(move || {
        let mut last_update = Instant::now();
        let mut last_keys_checked = 0;
        
        while !found_clone.load(AtomicOrdering::Relaxed) {
            thread::sleep(Duration::from_secs(1));
            
            let elapsed = last_update.elapsed();
            if elapsed >= Duration::from_secs(STATS_UPDATE_INTERVAL) {
                let current_keys_checked = keys_checked_clone.load(AtomicOrdering::Relaxed);
                let keys_since_last = current_keys_checked - last_keys_checked;
                let keys_per_second = keys_since_last as f64 / elapsed.as_secs_f64();
                
                println!("{}[{}] Verificadas: {} chaves ({:.2} M/s){}", 
                        colors::CYAN,
                        Local::now().format("%H:%M:%S"),
                        current_keys_checked,
                        keys_per_second / 1_000_000.0,
                        colors::RESET);
                
                if range_size_f64_clone != f64::MAX && range_size_f64_clone > 0.0 {
                    let progress_percentage = (current_keys_checked as f64 / range_size_f64_clone) * 100.0;
                    println!("{}Progresso: {:.8}% concluído{}", 
                            colors::YELLOW,
                            progress_percentage,
                            colors::RESET);
                            
                            // Estimativa de tempo restante
                            let keys_remaining = range_size_f64_clone - current_keys_checked as f64;
                            let time_remaining = keys_remaining / keys_per_second;
                            
                            let time_remaining_str = if time_remaining > 86400.0 {
                                format!("{:.2} dias", time_remaining / 86400.0)
                            } else if time_remaining > 3600.0 {
                                format!("{:.2} horas", time_remaining / 3600.0)
                            } else if time_remaining > 60.0 {
                                format!("{:.2} minutos", time_remaining / 60.0)
                            } else {
                                format!("{:.2} segundos", time_remaining)
                            };
                            
                            println!("{}Tempo restante estimado: {}{}", 
                                    colors::GREEN,
                                    time_remaining_str,
                                    colors::RESET);
                }
                
                last_update = Instant::now();
                last_keys_checked = current_keys_checked;
            }
        }
    });
    
    // Calcular o número de threads disponíveis
    let num_threads = rayon::current_num_threads();
    println!("{}Usando {} threads para busca paralela{}", colors::GREEN, num_threads, colors::RESET);
    
    // Calcular o tamanho de cada chunk
    let chunk_size = if !is_range_zero {
        &range_size / num_threads
    } else {
        BigUint::from(0u64)
    };
    
    // Preparar chunks para processamento paralelo
    let mut chunks = Vec::with_capacity(num_threads);
    let mut current = min_key.clone();
    
    for i in 0..num_threads {
        let next = if i == num_threads - 1 {
            // Último chunk vai até o final
            max_key.clone()
        } else {
            &current + &chunk_size
        };
        
        chunks.push((current.clone(), next.clone()));
        current = next;
    }
    
    // Processamento paralelo dos chunks
    chunks.par_iter().for_each(|(chunk_min, chunk_max)| {
        let mut current_key = chunk_min.clone();
        let mut batch = Vec::with_capacity(1024); // Usar um valor fixo em vez da constante
        
        while &current_key <= chunk_max {
            // Preencher o batch
            batch.clear();
            for _ in 0..1024 { // Usar um valor fixo em vez da constante
                if &current_key > chunk_max {
                    break;
                }
                
                batch.push(current_key.clone());
                current_key += 1u64;
            }
            
            // Processar o batch
            for key in &batch {
                // Se já encontrou a chave, sair do loop
                if found.load(AtomicOrdering::Relaxed) {
                    return;
                }
                
                // Conversão da chave para hash160
                let padded_key = pad_private_key_internal(key);
                
                // Processar apenas se a conversão para hash160 for bem-sucedida
                if let Ok(hash160) = private_key_to_hash160(&padded_key) {
                    // Verificar se corresponde ao hash160 alvo
                    if bytes_equal(&hash160, target_hash160) {
                        // Encontrou a chave!
                        let mut found_key_guard = found_key.lock().unwrap();
                        *found_key_guard = key.clone();
                        found.store(true, AtomicOrdering::Relaxed);
                        return;
                    }
                }
            }
            
            // Atualizar contagem de chaves verificadas
            keys_checked.fetch_add(batch.len() as u64, AtomicOrdering::Relaxed);
        }
    });
    
    // Aguardar thread de estatísticas
    let _ = stats_thread.join();
    
    // Verificar se encontrou
    if found.load(AtomicOrdering::Relaxed) {
        let found_key = found_key.lock().unwrap();
        let padded_key = pad_private_key_internal(&found_key);
        let key_hex = hex::encode(&padded_key);
        
        // Obter WIF e endereço com tratamento de erros
        let wif = match private_key_to_wif(&padded_key) {
            Ok(wif_str) => wif_str,
            Err(e) => format!("Erro ao gerar WIF: {:?}", e),
        };
        
        let address = match private_key_to_p2pkh_address(&padded_key) {
            Ok(addr_str) => addr_str,
            Err(e) => format!("Erro ao gerar endereço: {:?}", e),
        };
        
        println!("\n{}CHAVE ENCONTRADA!{}", colors::BOLD_GREEN, colors::RESET);
        println!("{}Chave privada (hex): {}{}", colors::GREEN, key_hex, colors::RESET);
        println!("{}Chave privada (WIF): {}{}", colors::GREEN, wif, colors::RESET);
        println!("{}Endereço Bitcoin: {}{}", colors::GREEN, address, colors::RESET);
        
        // Salvar resultados em arquivo
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let filename = format!("found_key_{}.txt", timestamp);
        
        if let Ok(mut file) = File::create(&filename) {
            let _ = writeln!(file, "CHAVE ENCONTRADA!");
            let _ = writeln!(file, "Chave privada (hex): {}", key_hex);
            let _ = writeln!(file, "Chave privada (WIF): {}", wif);
            let _ = writeln!(file, "Endereço Bitcoin: {}", address);
            println!("{}Resultados salvos em '{}'{}", colors::YELLOW, filename, colors::RESET);
        }
    } else {
        println!("\n{}Busca concluída. Chave privada não encontrada neste intervalo.{}", 
                colors::RED, colors::RESET);
    }
    
    // Estatísticas finais
    let total_keys_checked = keys_checked.load(AtomicOrdering::Relaxed);
    let elapsed = start_time.elapsed();
    let keys_per_second = total_keys_checked as f64 / elapsed.as_secs_f64();
    
    println!("{}Estatísticas finais:{}", colors::BOLD_YELLOW, colors::RESET);
    println!("{}Total de chaves verificadas: {}{}", colors::CYAN, total_keys_checked, colors::RESET);
    println!("{}Tempo total decorrido: {:.2} segundos{}", colors::CYAN, elapsed.as_secs_f64(), colors::RESET);
    println!("{}Velocidade média: {:.2} M chaves/segundo{}", 
            colors::CYAN, keys_per_second / 1_000_000.0, colors::RESET);
}

// Helper function to convert &[u8] to &[u8; 20]
fn convert_to_hash160_array(input: &[u8]) -> Result<[u8; 20], &'static str> {
    if input.len() != 20 {
        return Err("Invalid hash160 length");
    }
    
    let mut result = [0u8; 20];
    result.copy_from_slice(input);
    Ok(result)
}

/// Try to process a chunk with GPU, falling back to CPU if necessary
fn process_chunk_with_fallback(
    min_key: &BigUint,
    max_key: &BigUint, 
    target_hash160: &[u8],
    gpu_searcher: &mut OptionalGpuSearcher,
    batch_size: usize,
    chunk_index: usize,
    total_chunks: usize
) -> Option<String> {
    // Check if target is valid
    if target_hash160.len() != 20 {
        return None;
    }
    
    // Convert target_hash160 to [u8; 20] for GPU processing
    let target_array = match convert_to_hash160_array(target_hash160) {
        Ok(arr) => arr,
        Err(_) => return None, // Invalid target hash
    };
    
    // Try to convert the range to u64 for GPU processing
    // If the range is too large, we'll split it further or use CPU
    let min_key_bits = min_key.bits();
    let max_key_bits = max_key.bits();
    
    let range_too_large_for_direct_gpu = min_key_bits > 63 || max_key_bits > 63;
    
    // Calculate range size
    let range_size = max_key - min_key;
    let range_bits = range_size.bits();
    
    // Format user-friendly output showing the range
    let min_key_hex = format!("{:x}", min_key);
    let max_key_hex = format!("{:x}", max_key);
    
    // Only display a portion of very large keys to avoid cluttering the console
    let min_display = if min_key_hex.len() > 20 {
        format!("{}...", &min_key_hex[0..16])
    } else {
        min_key_hex.clone()
    };
    
    let max_display = if max_key_hex.len() > 20 {
        format!("{}...", &max_key_hex[0..16])
    } else {
        max_key_hex.clone()
    };
    
    println!("{}Chunk {}/{}: Range {} - {} ({} bits){}", 
             crate::colors::CYAN, chunk_index, total_chunks, 
             min_display, max_display, range_bits,
             crate::colors::RESET);
    
    // Try GPU for ranges within reasonable size
    #[cfg(feature = "opencl")]
    if let Some(searcher) = gpu_searcher.as_mut() {
        if !range_too_large_for_direct_gpu && range_bits < 64 {
            // Range fits within u64, can use GPU directly
            let min_u64 = min_key.to_u64().unwrap_or(0);
            let max_u64 = max_key.to_u64().unwrap_or(u64::MAX);
            
            println!("{}GPU iniciando busca no range: {} - {}{}", 
                     crate::colors::GREEN, min_u64, max_u64, crate::colors::RESET);
            
            match searcher.search_direct(&target_array, min_u64, max_u64, batch_size) {
                Ok(found_keys) => {
                    if !found_keys.is_empty() {
                        // Process found keys
                        for key in found_keys {
                            // Convert the u64 key to a hex string
                            let key_hex = format!("{:x}", key);
                            let key_bytes = match hex::decode(&key_hex) {
                                Ok(bytes) => bytes,
                                Err(_) => continue,
                            };
                            
                            // Validate the key
                            if validate_private_key_for_hash160(&key_bytes, target_hash160) {
                                return Some(key_hex);
                            }
                        }
                    }
                    return None; // No valid key found in this range
                },
                Err(e) => {
                    println!("{}GPU error: {}, switching to CPU for this chunk{}", 
                             crate::colors::YELLOW, e, crate::colors::RESET);
                    // Fall through to CPU processing
                }
            }
        } else {
            // Range is too large for direct GPU processing
            println!("{}Range muito grande para GPU ({} bits), processando com CPU{}", 
                     crate::colors::YELLOW, range_bits, crate::colors::RESET);
        }
    }
    
    // CPU processing for this chunk
    println!("{}Processando com CPU: {} - {}{}", 
             crate::colors::BLUE, min_display, max_display, crate::colors::RESET);
    
    // Process the chunk on CPU, breaking it into smaller pieces if needed
    let max_cpu_chunk_size = BigUint::from(1_000_000_000u64); // 1 billion keys per CPU sub-chunk
    
    if range_size > max_cpu_chunk_size {
        // Break into smaller sub-chunks for CPU
        let num_subchunks = (range_size.clone() + max_cpu_chunk_size.clone() - BigUint::from(1u32))
            / max_cpu_chunk_size.clone();
        
        let subchunk_size = range_size.clone() / num_subchunks.clone();
        
        println!("{}CPU: Dividindo range em {} sub-chunks{}", 
                 crate::colors::BLUE, num_subchunks, crate::colors::RESET);
        
        let mut current = min_key.clone();
        
        for i in 0..num_subchunks.to_u64().unwrap_or(1) {
            let subchunk_end = if i == num_subchunks.to_u64().unwrap_or(1) - 1 {
                max_key.clone()
            } else {
                current.clone() + subchunk_size.clone()
            };
            
            println!("{}CPU sub-chunk {}/{}{}", 
                     crate::colors::BLUE, i+1, num_subchunks, crate::colors::RESET);
            
            let result = search_key_range_cpu(&current, &subchunk_end, target_hash160, batch_size);
            
            if let Some(key) = result {
                return Some(key);
            }
            
            current = subchunk_end + BigUint::from(1u32);
        }
        
        return None;
    } else {
        // Process the entire chunk directly on CPU
        search_key_range_cpu(min_key, max_key, target_hash160, batch_size)
    }
}

// Process a range of keys directly on CPU
fn search_key_range_cpu(
    min_key: &BigUint,
    max_key: &BigUint,
    target_hash160: &[u8],
    batch_size: usize
) -> Option<String> {
    // Use the optimized CPU approach
    let mut current_key = min_key.clone();
    
    // Process keys in batches
    let mut keys_buffer = Vec::with_capacity(batch_size);
    let increment = BigUint::from(batch_size);
    
    while current_key <= *max_key {
        // Clear the buffer for this batch
        keys_buffer.clear();
        
        // Calculate the end of this batch
        let batch_end = std::cmp::min(
            &current_key + &increment, 
            max_key.clone() + BigUint::from(1u32)
        );
        
        // Fill the buffer with keys for this batch
        let mut key = current_key.clone();
        while key < batch_end {
            keys_buffer.push(key.clone());
            key += 1u32;
        }
        
        // Process this batch
        for key in &keys_buffer {
            // Count this key as checked
            crate::performance::increment_keys_checked(1);
            
            // Convert to bytes and check
            let key_hex = format!("{:x}", key);
            let key_bytes = match hex::decode(&key_hex) {
                Ok(bytes) => bytes,
                Err(_) => continue,
            };
            
            // Validate the key
            if validate_private_key_for_hash160(&key_bytes, target_hash160) {
                return Some(key_hex);
            }
        }
        
        // Move to the next batch
        current_key = batch_end;
    }
    
    None
}

/// Search for a private key optimized version with GPU support
#[allow(clippy::too_many_arguments)]
pub fn search_for_private_key_optimized(
    search_ranges: &Vec<(BigUint, BigUint)>,
    target_hash160: &[u8], 
    batch_size: usize,
    mut gpu_searcher: OptionalGpuSearcher,
) -> Option<String> {
    // Initialize a start time for this search
    let search_start_time = Instant::now();
    
    let mut found_key: Option<String> = None;
    
    // Reset the counter
    KEYS_CHECKED.store(0, std::sync::atomic::Ordering::SeqCst);
    
    // Check if target hash160 is valid (20 bytes)
    if target_hash160.len() != 20 {
        println!("{}Erro: Hash160 inválido, tamanho deve ser 20 bytes{}", 
                 crate::colors::RED, crate::colors::RESET);
        return None;
    }
    
    // Create a fixed-size array for the target hash
    let mut target_array: [u8; 20] = [0; 20];
    target_array.copy_from_slice(target_hash160);
    
    // For better feedback, calculate the total number of ranges
    let total_ranges = search_ranges.len();
    
    // Track GPU vs CPU usage (for statistics)
    let _gpu_processed_chunks = std::sync::atomic::AtomicUsize::new(0);
    let _cpu_processed_chunks = std::sync::atomic::AtomicUsize::new(0);
    
    println!("{}Iniciando busca com {} chunks...{}", 
             crate::colors::BOLD_GREEN, total_ranges, crate::colors::RESET);
    
    // Start search for all ranges
    for (i, (min, max)) in search_ranges.iter().enumerate() {
        // Try to process this chunk with available methods
        let chunk_result = process_chunk_with_fallback(
            min, max, target_hash160, &mut gpu_searcher, batch_size, i+1, total_ranges
        );
        
        if let Some(key) = chunk_result {
            found_key = Some(key);
            break;
        }
        
        // Update progress
        let keys_checked = KEYS_CHECKED.load(std::sync::atomic::Ordering::SeqCst);
        let elapsed = search_start_time.elapsed().as_secs();
        
        if elapsed > 0 {
            // Calculate speed
            let speed = keys_checked as f64 / elapsed as f64;
            
            // Only show progress after at least 1 second
            println!("{}Progresso: {}/{} chunks ({:.2}%) | {:.2}M chaves/s | {} chaves verificadas{}", 
                     crate::colors::CYAN, 
                     i+1, total_ranges, 
                     (i+1) as f64 / total_ranges as f64 * 100.0,
                     speed / 1_000_000.0,
                     keys_checked,
                     crate::colors::RESET);
            
            // Estimativa de tempo restante
            let percentage = (i+1) as f64 / total_ranges as f64 * 100.0;
            let percentage_remaining = 100.0 - percentage;
            
            // Use the current time instead of START_TIME
            let estimated_total_seconds = (elapsed as f64 / percentage) * 100.0;
            let remaining_seconds = estimated_total_seconds - elapsed as f64;
            
            let remaining_hours = (remaining_seconds / 3600.0) as u64;
            let remaining_minutes = ((remaining_seconds % 3600.0) / 60.0) as u64;
            
            println!("{}Tempo restante estimado: {:02}:{:02}:{:02} ({:.1}% restante){}", 
                    crate::colors::YELLOW, 
                    remaining_hours, remaining_minutes, (remaining_seconds % 60.0) as u64,
                    percentage_remaining,
                    crate::colors::RESET);
        }
    }
    
    found_key
}

#[cfg(feature = "opencl")]
pub struct GpuSearchContext {
    gpu_searcher: OptionalGpuSearcher
}

pub fn search_for_private_key_optimized_legacy(
    chunks: &[(BigUint, BigUint)], 
    target_hash160: &[u8], 
    batch_size: usize,
    gpu_searcher: OptionalGpuSearcher
) {
    let start_time = Instant::now();
    let found = Arc::new(AtomicBool::new(false));
    let found_key = Arc::new(Mutex::new(BigUint::zero()));
    
    // Thread para exibir estatísticas periódicas
    let found_clone = Arc::clone(&found);
    let stats_thread = thread::spawn(move || {
        let mut last_update = Instant::now();
        let mut last_keys_checked = 0;
        
        while !found_clone.load(AtomicOrdering::Relaxed) {
            thread::sleep(Duration::from_secs(1));
            
            let elapsed = last_update.elapsed();
            if elapsed >= Duration::from_secs(STATS_UPDATE_INTERVAL) {
                // Obter contagem atual de chaves verificadas do módulo de performance
                let current_keys_checked = performance::get_keys_checked() as u64;
                let keys_since_last = current_keys_checked - last_keys_checked;
                let keys_per_second = keys_since_last as f64 / elapsed.as_secs_f64();
                
                println!("{}[{}] Verificadas: {} chaves ({:.2} M/s){}", 
                        colors::CYAN,
                        Local::now().format("%H:%M:%S"),
                        current_keys_checked,
                        keys_per_second / 1_000_000.0,
                        colors::RESET);
                
                // Velocidade média global
                if elapsed.as_secs() > 0 {
                    let overall_time = start_time.elapsed().as_secs_f64();
                    let overall_speed = current_keys_checked as f64 / overall_time;
                    
                    println!("{}Velocidade média global: {:.2} M/s{}", 
                            colors::YELLOW,
                            overall_speed / 1_000_000.0,
                            colors::RESET);
                }
                
                last_update = Instant::now();
                last_keys_checked = current_keys_checked;
            }
        }
    });

    // Se temos um GPU searcher, usar ele para processar alguns chunks
    #[cfg(feature = "opencl")]
    if let Some(searcher) = gpu_searcher {
        println!("{}Iniciando busca com GPU...{}", colors::BOLD_GREEN, colors::RESET);
        
        // Converter o hash160 de destino para o formato esperado pela GPU
        let mut target_set = HashSet::new();
        let mut hash_arr = [0u8; 20];
        hash_arr.copy_from_slice(target_hash160);
        target_set.insert(hash_arr);
        
        // Processar cada chunk com a GPU
        for (chunk_min, chunk_max) in chunks {
            // Se já encontrou a chave, não iniciar novos chunks
            if found.load(AtomicOrdering::Relaxed) {
                break;
            }
            
            // Converter BigUint para u64 para usar na GPU
            // Nota: isso só funciona se os valores couberem em u64
            let min_u64 = match chunk_min.to_u64() {
                Some(val) => val,
                None => {
                    println!("{}Aviso: Valor muito grande para GPU, pulando chunk{}", 
                            colors::YELLOW, colors::RESET);
                    continue;
                }
            };
            
            let max_u64 = match chunk_max.to_u64() {
                Some(val) => val,
                None => {
                    println!("{}Aviso: Valor muito grande para GPU, pulando chunk{}", 
                            colors::YELLOW, colors::RESET);
                    continue;
                }
            };
            
            println!("{}Processando chunk na GPU: {} a {}{}", 
                     colors::CYAN, min_u64, max_u64, colors::RESET);
            
            // Executar busca na GPU
            match searcher.search(&target_set, min_u64, max_u64, batch_size) {
                Ok(found_keys) => {
                    // Verificar se encontrou alguma chave
                    if !found_keys.is_empty() {
                        let found_key_value = found_keys[0];
                        println!("{}GPU encontrou chave candidata: {}{}", 
                                colors::GREEN, found_key_value, colors::RESET);
                        
                        // Verificar a chave na CPU para confirmar
                        let key = BigUint::from(found_key_value);
                        let padded_key = pad_private_key(&key);
                        
                        if let Ok(hash160) = private_key_to_hash160(&padded_key) {
                            if bytes_equal(&hash160, target_hash160) {
                                // Encontrou a chave!
                                let mut found_key_guard = found_key.lock().unwrap();
                                *found_key_guard = key;
                                found.store(true, AtomicOrdering::Relaxed);
                                break;
                            }
                        }
                    }
                    
                    // Atualizar contagem de chaves verificadas
                    let range_size = max_u64 - min_u64;
                    performance::increment_keys_checked(range_size as usize);
                },
                Err(e) => {
                    println!("{}Erro na busca GPU: {}{}", colors::RED, e, colors::RESET);
                    println!("{}Continuando com CPU para este chunk{}", colors::YELLOW, colors::RESET);
                    
                    // Processar este chunk com CPU
                    process_chunk_cpu(chunk_min, chunk_max, target_hash160, batch_size, &found, &found_key);
                }
            }
        }
    } else {
        // Número de chunks para processamento paralelo
        println!("{}Usando {} chunks para processamento paralelo em CPU{}", 
                colors::GREEN, chunks.len(), colors::RESET);
        
        // Processamento paralelo dos chunks predefinidos com CPU
        chunks.par_iter().for_each(|(chunk_min, chunk_max)| {
            // Se já encontrou a chave, não iniciar novos chunks
            if found.load(AtomicOrdering::Relaxed) {
                return;
            }
            
            process_chunk_cpu(chunk_min, chunk_max, target_hash160, batch_size, &found, &found_key);
        });
    }
    
    #[cfg(not(feature = "opencl"))]
    {
        // Número de chunks para processamento paralelo
        println!("{}Usando {} chunks para processamento paralelo em CPU{}", 
                colors::GREEN, chunks.len(), colors::RESET);
        
        // Processamento paralelo dos chunks predefinidos com CPU
        chunks.par_iter().for_each(|(chunk_min, chunk_max)| {
            // Se já encontrou a chave, não iniciar novos chunks
            if found.load(AtomicOrdering::Relaxed) {
                return;
            }
            
            process_chunk_cpu(chunk_min, chunk_max, target_hash160, batch_size, &found, &found_key);
        });
    }
    
    // Aguardar thread de estatísticas
    let _ = stats_thread.join();
    
    // Verificar se encontrou
    if found.load(AtomicOrdering::Relaxed) {
        let found_key = found_key.lock().unwrap();
        let padded_key = pad_private_key(&found_key);
        let key_hex = hex::encode(&padded_key);
        
        // Obter WIF e endereço com tratamento de erros
        let wif = match private_key_to_wif(&padded_key) {
            Ok(wif_str) => wif_str,
            Err(e) => format!("Erro ao gerar WIF: {:?}", e),
        };
        
        let address = match private_key_to_p2pkh_address(&padded_key) {
            Ok(addr_str) => addr_str,
            Err(e) => format!("Erro ao gerar endereço: {:?}", e),
        };
        
        println!("\n{}CHAVE ENCONTRADA!{}", colors::BOLD_GREEN, colors::RESET);
        println!("{}Chave privada (hex): {}{}", colors::GREEN, key_hex, colors::RESET);
        println!("{}Chave privada (WIF): {}{}", colors::GREEN, wif, colors::RESET);
        println!("{}Endereço Bitcoin: {}{}", colors::GREEN, address, colors::RESET);
        
        // Salvar resultados em arquivo
        let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
        let filename = format!("found_key_{}.txt", timestamp);
        
        if let Ok(mut file) = File::create(&filename) {
            let _ = writeln!(file, "CHAVE ENCONTRADA!");
            let _ = writeln!(file, "Chave privada (hex): {}", key_hex);
            let _ = writeln!(file, "Chave privada (WIF): {}", wif);
            let _ = writeln!(file, "Endereço Bitcoin: {}", address);
            println!("{}Resultados salvos em '{}'{}", colors::YELLOW, filename, colors::RESET);
        }
    } else {
        println!("\n{}Busca concluída. Chave privada não encontrada neste intervalo.{}", 
                colors::RED, colors::RESET);
    }
    
    // Estatísticas finais
    let total_keys_checked = performance::get_keys_checked();
    let elapsed = start_time.elapsed();
    let keys_per_second = total_keys_checked as f64 / elapsed.as_secs_f64();
    
    println!("{}Estatísticas finais:{}", colors::BOLD_YELLOW, colors::RESET);
    println!("{}Total de chaves verificadas: {}{}", colors::CYAN, total_keys_checked, colors::RESET);
    println!("{}Tempo total decorrido: {:.2} segundos{}", colors::CYAN, elapsed.as_secs_f64(), colors::RESET);
    println!("{}Velocidade média: {:.2} M chaves/segundo{}", 
            colors::CYAN, keys_per_second / 1_000_000.0, colors::RESET);
}

/// Process a chunk with CPU
fn process_chunk_cpu(
    chunk_min: &BigUint,
    chunk_max: &BigUint,
    target_hash160: &[u8],
    batch_size: usize,
    found: &Arc<AtomicBool>,
    found_key: &Arc<Mutex<BigUint>>
) {
    // Skip if we already found the key
    if found.load(AtomicOrdering::Relaxed) {
        return;
    }
    
    println!("{}Processando chunk na CPU: {} a {}{}", 
             crate::colors::BLUE,
             chunk_min.to_str_radix(16),
             chunk_max.to_str_radix(16),
             crate::colors::RESET);
    
    // Use the optimized CPU approach with batches
    let mut current_key = chunk_min.clone();
    
    // Process keys in batches
    let mut keys_buffer = Vec::with_capacity(batch_size);
    let increment = BigUint::from(batch_size);
    
    while &current_key <= chunk_max {
        // Skip if we already found the key
        if found.load(AtomicOrdering::Relaxed) {
            return;
        }
        
        // Clear the buffer for this batch
        keys_buffer.clear();
        
        // Calculate the end of this batch
        let batch_end = std::cmp::min(
            &current_key + &increment, 
            chunk_max.clone() + BigUint::from(1u32)
        );
        
        // Fill the buffer with keys for this batch
        let mut key = current_key.clone();
        while key < batch_end {
            keys_buffer.push(key.clone());
            key += 1u32;
        }
        
        // Process this batch
        for key in &keys_buffer {
            // Skip if we already found the key
            if found.load(AtomicOrdering::Relaxed) {
                return;
            }
            
            // Count this key as checked
            crate::performance::increment_keys_checked(1);
            
            // Convert to bytes
            let padded_key = pad_private_key_internal(key);
            
            // Generate Hash160
            if let Ok(hash160) = private_key_to_hash160(&padded_key) {
                // Check if hash160 matches target
                if bytes_equal(&hash160, target_hash160) {
                    // We found the key!
                    let mut found_key_guard = found_key.lock().unwrap();
                    *found_key_guard = key.clone();
                    found.store(true, AtomicOrdering::Relaxed);
                    return;
                }
            }
        }
        
        // Move to the next batch
        current_key = batch_end;
    }
} 