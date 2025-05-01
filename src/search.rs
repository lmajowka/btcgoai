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

use crate::bitcoin::{pad_private_key, private_key_to_hash160, private_key_to_wif, private_key_to_p2pkh_address};
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

/// Search for a private key optimized version with GPU support
#[allow(clippy::too_many_arguments)]
pub fn search_for_private_key_optimized(
    search_ranges: &Vec<(BigUint, BigUint)>,
    target_hash160: &[u8],
    batch_size: usize,
    mut gpu_searcher: OptionalGpuSearcher,
) -> Option<String> {
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
    let mut target_hash_arr = [0u8; 20];
    target_hash_arr.copy_from_slice(target_hash160);
    
    // Determine if GPU is available
    #[cfg(feature = "opencl")]
    let gpu_available = gpu_searcher.is_some();
    
    #[cfg(not(feature = "opencl"))]
    let gpu_available = false;
    
    // Create channel to communicate between threads
    let (tx, rx) = std::sync::mpsc::channel();
    
    // Flag to track if GPU has failed or is unavailable
    let gpu_failed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(!gpu_available));
    
    // Store progress information
    let total_ranges = search_ranges.len();
    let search_start_time = std::time::Instant::now();
    let mut last_status_time = std::time::Instant::now();
    
    // Count of completed chunks
    let completed_chunks = std::sync::atomic::AtomicUsize::new(0);
    
    // Start CPU search for all ranges
    for (i, (min, max)) in search_ranges.iter().enumerate() {
        // Skip if we already found the key
        if let Ok(Some(_)) = rx.try_recv() {
            break;
        }
        
        // Try GPU search for suitable ranges first
        #[cfg(feature = "opencl")]
        let processed_by_gpu = {
            // Only try GPU if it's available and hasn't failed previously
            if gpu_available && !gpu_failed.load(std::sync::atomic::Ordering::SeqCst) {
                // Only process with GPU if range fits in u64
                if let (Some(min_u64), Some(max_u64)) = (min.to_u64(), max.to_u64()) {
                    // Only process with GPU if range isn't too large
                    if max_u64 - min_u64 <= 1_000_000_000_000u64 {
                        // Process with GPU if possible
                        if let Some(ref mut searcher) = gpu_searcher {
                            println!("GPU procurando chunk {}/{}", i+1, total_ranges);
                            
                            // Try to search using GPU
                            match searcher.search_direct(&target_hash_arr, min_u64, max_u64, batch_size) {
                                Ok(found_keys) => {
                                    // Check if any keys were found
                                    if !found_keys.is_empty() {
                                        // Send the first found key back
                                        let key_hex = format!("{:x}", found_keys[0]);
                                        found_key = Some(key_hex);
                                        // Skip CPU search
                                        true
                                    } else {
                                        // Update the keys checked counter
                                        let range_size = max_u64 - min_u64;
                                        KEYS_CHECKED.fetch_add(range_size as usize / 10, std::sync::atomic::Ordering::Relaxed);
                                        completed_chunks.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                                        // Skip CPU search
                                        true
                                    }
                                },
                                Err(e) => {
                                    // Log the error
                                    println!("Erro GPU (chunk {}/{}): {}", i+1, total_ranges, e);
                                    
                                    // Increment failure counter (static to persist across chunks)
                                    static GPU_FAILURES: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
                                    let failures = GPU_FAILURES.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                                    
                                    // If too many failures, mark GPU as failed
                                    if failures >= 3 {
                                        println!("{}Muitas falhas na GPU. Revertendo para CPU.{}", 
                                                crate::colors::YELLOW, crate::colors::RESET);
                                        gpu_failed.store(true, std::sync::atomic::Ordering::SeqCst);
                                    }
                                    
                                    // Process with CPU
                                    false
                                }
                            }
                        } else {
                            // GPU searcher is not available, process with CPU
                            false
                        }
                    } else {
                        // Range too large for GPU
                        false
                    }
                } else {
                    // Range doesn't fit in u64, process with CPU
                    false
                }
            } else {
                // GPU not available or has failed
                false
            }
        };
        
        #[cfg(not(feature = "opencl"))]
        let processed_by_gpu = false;
        
        // If not processed by GPU, process with CPU
        if !processed_by_gpu {
            // Clone values for the CPU thread
            let tx_clone = tx.clone();
            let min_clone = min.clone();
            let max_clone = max.clone();
            let target_clone = target_hash_arr;
            let i_clone = i;
            
            // Spawn a CPU worker thread
            std::thread::spawn(move || {
                println!("CPU procurando chunk {}/{}", i_clone+1, total_ranges);
                
                if let Some(key) = search_range_for_private_key(&min_clone, &max_clone, &target_clone, batch_size) {
                    let _ = tx_clone.send(Some((i_clone, key)));
                } else {
                    let _ = tx_clone.send(None);
                }
            });
        }
    }
    
    // Drop sender to avoid deadlock
    drop(tx);
    
    // If we already found a key with GPU, return it without waiting for CPU threads
    if found_key.is_some() {
        return found_key;
    }
    
    // Display progress and check for results from CPU threads
    let mut found_key_value: Option<String> = None;
    
    // Loop until we get a result or all threads finish
    while found_key_value.is_none() {
        // Check for results from any thread
        match rx.recv_timeout(std::time::Duration::from_secs(1)) {
            Ok(Some((_, key))) => {
                found_key_value = Some(key);
                break;
            },
            Ok(None) => {
                // One thread finished without finding
                completed_chunks.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                
                // If all chunks are done, exit
                if completed_chunks.load(std::sync::atomic::Ordering::SeqCst) >= total_ranges {
                    break;
                }
            },
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Just a timeout, continue
            },
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                // All senders disconnected, we're done
                break;
            }
        }
        
        // Update status display every 30 seconds
        let now = std::time::Instant::now();
        if now.duration_since(last_status_time).as_secs() >= 30 {
            last_status_time = now;
            
            // Calculate speed
            let elapsed = now.duration_since(search_start_time).as_secs();
            if elapsed > 0 {
                let keys_checked = KEYS_CHECKED.load(std::sync::atomic::Ordering::Relaxed);
                let speed = keys_checked as f64 / elapsed as f64 / 1_000_000.0;
                
                // Format time
                let hours = elapsed / 3600;
                let minutes = (elapsed % 3600) / 60;
                let seconds = elapsed % 60;
                
                println!("[{:02}:{:02}:{:02}] Verificadas: {} chaves ({:.2} M/s)",
                         hours, minutes, seconds, keys_checked, speed);
                
                // Calculate the global average speed over the entire session
                let total_speed = keys_checked as f64 / elapsed as f64 / 1_000_000.0;
                println!("Velocidade média global: {:.2} M/s", total_speed);
                
                // Display completion percentage
                let completed = completed_chunks.load(std::sync::atomic::Ordering::SeqCst);
                let percentage = (completed as f64 / total_ranges as f64) * 100.0;
                println!("Progresso: {}/{} chunks ({:.1}%)", 
                         completed, total_ranges, percentage);
            }
        }
    }
    
    // Return either the found key from GPU or from CPU threads
    found_key.or(found_key_value)
}

/// Search a specific range for a private key
pub fn search_range_for_private_key(
    min: &BigUint,
    max: &BigUint,
    target_hash160: &[u8; 20],
    batch_size: usize
) -> Option<String> {
    // Creating a range from min to max for iteration
    let range_diff = max - min;
    
    // Batch processing for better performance
    let mut current = min.clone();
    
    while current <= *max {
        // Calculate batch end
        let batch_end = if range_diff > batch_size.into() {
            let next_batch = &current + batch_size;
            if next_batch > *max {
                max.clone()
            } else {
                next_batch
            }
        } else {
            max.clone()
        };
        
        // Process keys in the current batch - use a loop with increment instead of range
        let mut key = current.clone();
        while key <= batch_end {
            // Check if this key produces the target hash160
            let padded_key = pad_private_key_internal(&key);
            
            // Use crate::bitcoin to access the function
            if let Ok(hash160) = crate::bitcoin::private_key_to_hash160(&padded_key) {
                // Compare with target hash - convert Vec<u8> to fixed array reference
                if hash160 == target_hash160[..] {
                    // Found the key!
                    return Some(format!("{:x}", key));
                }
            }
            
            // Increment key
            key += 1u32;
            
            // Update counter for statistics
            KEYS_CHECKED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        
        // Move to the next batch
        current = batch_end + 1u32;
    }
    
    // If execution reaches here, key wasn't found
    None
}

// Function to process a chunk with CPU
fn process_chunk_cpu(
    min: &BigUint,
    max: &BigUint,
    target_hash160: &[u8],
    batch_size: usize,
    found: &Arc<AtomicBool>,
    found_key: &Arc<Mutex<BigUint>>
) {
    // Verificar se este chunk está dentro do intervalo minimo possível
    if max < min {
        // Intervalo inválido, ignorar
        return;
    }
    
    // Criar iterador para este intervalo
    let mut current = min.clone();
    let mut batch_count = 0;
    
    while current <= *max && !found.load(AtomicOrdering::Relaxed) {
        // Calcular fim do batch atual
        let batch_end = if &current + batch_size <= *max {
            &current + batch_size
        } else {
            max.clone()
        };
        
        // Calcular tamanho do batch para estatísticas
        let batch_len = (&batch_end - &current + 1u32).to_usize().unwrap_or(0);
        
        // Processar batch somente se necessário
        if batch_len > 0 && !found.load(AtomicOrdering::Relaxed) {
            let key_start = current.clone();
            let mut key = key_start.clone();
            
            while key <= batch_end {
                // Verificar apenas se ainda não encontrou
                if found.load(AtomicOrdering::Relaxed) {
                    break;
                }
                
                // Conversão da chave para hash160
                let padded_key = pad_private_key_internal(&key);
                
                // Processar apenas se a conversão para hash160 for bem-sucedida
                if let Ok(hash160) = crate::bitcoin::private_key_to_hash160(&padded_key) {
                    // Verificar se corresponde ao alvo
                    if bytes_equal(&hash160, target_hash160) {
                        // Encontrou a chave!
                        let mut found_key_guard = found_key.lock().unwrap();
                        *found_key_guard = key.clone();
                        found.store(true, AtomicOrdering::Relaxed);
                        break;
                    }
                }
                
                // Próxima chave
                key += 1u32;
            }
            
            // Atualizar contador para estatísticas
            KEYS_CHECKED.fetch_add(batch_len, AtomicOrdering::Relaxed);
            
            // Debug a cada 10 batches (não muito frequente para não impactar performance)
            batch_count += 1;
            if batch_count % 10 == 0 {
                // Calcular progresso aproximado
                let progress = if max > min {
                    let total_range = max - min;
                    let processed = &current - min;
                    (processed.to_f64().unwrap_or(0.0) / total_range.to_f64().unwrap_or(1.0) * 100.0).min(100.0)
                } else {
                    100.0
                };
                
                print!("\rProgresso: {:.2}% | Verificando: {}", progress, current);
                std::io::stdout().flush().unwrap_or(());
            }
        }
        
        // Avançar para próximo batch
        current = batch_end + 1u32;
    }
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