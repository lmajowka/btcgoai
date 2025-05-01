use std::fs::File;
use std::io::Write;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use std::thread;

use num_bigint::{BigUint, RandomBits};
use num_traits::{One, Zero, ToPrimitive};
use rand::{Rng, thread_rng};
use chrono::Local;
use rayon::prelude::*;
use hex;

use crate::bitcoin::{pad_private_key, private_key_to_hash160, private_key_to_wif, private_key_to_p2pkh_address};
use crate::colors;

// Tamanho do batch para processamento em lotes
const BATCH_SIZE: usize = 1024;
// Frequência de atualização das estatísticas (em segundos)
const STATS_UPDATE_INTERVAL: u64 = 5;

// Compare two byte slices for equality - versão otimizada
fn bytes_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    
    // SIMD-friendly comparison - better cache locality
    let chunks_a = a.chunks_exact(16);
    let remainder_a = chunks_a.remainder();
    let chunks_b = b.chunks_exact(16);
    let remainder_b = chunks_b.remainder();
    
    // Compare 16-byte chunks
    let chunks_equal = chunks_a.zip(chunks_b)
        .all(|(a_chunk, b_chunk)| a_chunk == b_chunk);
        
    // Compare remainder
    let remainder_equal = remainder_a == remainder_b;
    
    chunks_equal && remainder_equal
}

// Uma struct para representar uma chave privada e seu hash160
struct KeyBatch {
    private_keys: Vec<Vec<u8>>,
    hash160s: Vec<Vec<u8>>,
}

impl KeyBatch {
    fn new(capacity: usize) -> Self {
        KeyBatch {
            private_keys: Vec::with_capacity(capacity),
            hash160s: Vec::with_capacity(capacity),
        }
    }
    
    fn clear(&mut self) {
        self.private_keys.clear();
        self.hash160s.clear();
    }
}

// Search for a private key that corresponds to the target hash160
// within the given range (min_key to max_key) using multiple threads
pub fn search_for_private_key(min_key: &BigUint, max_key: &BigUint, target_hash160: &[u8]) {
    // Determine the number of threads to use based on available CPU cores
    let num_cpu = num_cpus::get();
    let num_workers = num_cpu * 2; // Use 2x the number of CPUs for best performance
    println!("{}Starting key search with {} workers...{}", colors::BLUE, num_workers, colors::RESET);
    
    // Clone the min and max values to be owned rather than borrowed
    let min_key = min_key.clone();
    let max_key = max_key.clone();
    
    // Determine the limit for iterations to prevent infinite loops
    let diff = &max_key - &min_key;
    println!("{}Keyspace size: {} keys{}", colors::BLUE, diff.to_string(), colors::RESET);
    
    // Variables for synchronization and tracking
    let found_match = Arc::new(AtomicBool::new(false));
    let found_key = Arc::new(Mutex::new(Vec::new()));
    let found_hash160 = Arc::new(Mutex::new(Vec::new()));
    let total_iterations = Arc::new(AtomicU64::new(0));
    let last_key_checked = Arc::new(Mutex::new(BigUint::zero()));
    
    // Generate a random starting point within the range
    let rand_offset: BigUint = thread_rng().sample(RandomBits::new(diff.bits() as u64));
    let random_start: BigUint = &min_key + rand_offset;
    
    println!("{}Starting from random position within range...{}", colors::BLUE, colors::RESET);
    let random_start_hex = hex::encode(random_start.to_bytes_be());
    println!("{}Random start point: {}{}{}", colors::CYAN, colors::BOLD_CYAN, random_start_hex, colors::RESET);
    
    // Divide the keyspace into chunks for each worker
    let chunk_size = if diff.is_zero() {
        BigUint::one()
    } else {
        &diff / num_workers
    };
    
    // Setup for progress reporting
    let start_time = Instant::now();
    
    // Create a thread to report progress periodically
    let total_iterations_clone = Arc::clone(&total_iterations);
    let found_match_clone = Arc::clone(&found_match);
    let last_key_checked_clone = Arc::clone(&last_key_checked);
    let diff_clone = diff.clone();
    let min_key_clone = min_key.clone();
    
    thread::spawn(move || {
        let mut last_count = 0u64;
        let mut last_time = Instant::now();
        
        while !found_match_clone.load(AtomicOrdering::Relaxed) {
            thread::sleep(Duration::from_secs(STATS_UPDATE_INTERVAL));
            
            // If a match was found while we were sleeping, exit
            if found_match_clone.load(AtomicOrdering::Relaxed) {
                return;
            }
            
            // Calculate and report stats
            let current_time = Instant::now();
            let elapsed_since_last = current_time.duration_since(last_time).as_secs_f64();
            let total_elapsed = current_time.duration_since(start_time).as_secs_f64();
            
            let current_count = total_iterations_clone.load(AtomicOrdering::Relaxed);
            let keys_since_last = current_count.saturating_sub(last_count);
            
            // Calculate rates
            let keys_per_second_recent = keys_since_last as f64 / elapsed_since_last;
            let keys_per_second_avg = current_count as f64 / total_elapsed;
            
            // Calcular progresso percentual
            let last_key = last_key_checked_clone.lock().unwrap().clone();
            let progress_key = if last_key >= min_key_clone {
                &last_key - &min_key_clone
            } else {
                BigUint::zero()
            };
            
            // Calcular progresso e tempo restante estimado
            let progress_ratio = if !diff_clone.is_zero() {
                progress_key.to_f64().unwrap() / diff_clone.to_f64().unwrap()
            } else {
                0.0
            };
            
            let progress_percent = progress_ratio * 100.0;
            
            // Estimar tempo restante
            let remaining_seconds = if keys_per_second_recent > 0.0 {
                let remaining_keys = diff_clone.to_f64().unwrap() - (current_count as f64);
                remaining_keys / keys_per_second_recent
            } else {
                0.0
            };
            
            let remaining_hours = remaining_seconds / 3600.0;
            let remaining_days = remaining_hours / 24.0;
            let remaining_years = remaining_days / 365.25;
            
            let remaining_time = if remaining_years > 1.0 {
                format!("{:.2} anos", remaining_years)
            } else if remaining_days > 1.0 {
                format!("{:.2} dias", remaining_days)
            } else if remaining_hours > 1.0 {
                format!("{:.2} horas", remaining_hours)
            } else {
                format!("{:.2} minutos", remaining_seconds / 60.0)
            };
            
            // Update for next iteration
            last_count = current_count;
            last_time = current_time;
            
            // Get the last key checked
            let last_key_hex = hex::encode(last_key.to_bytes_be());
            
            // Exibir estatísticas
            println!("\n{}Estatísticas de busca:{}", colors::BOLD_CYAN, colors::RESET);
            println!("{}Total de chaves verificadas: {}", colors::CYAN, current_count);
            println!("{}Velocidade atual: {:.2} M chaves/s", colors::CYAN, keys_per_second_recent / 1_000_000.0);
            println!("{}Velocidade média: {:.2} M chaves/s", colors::CYAN, keys_per_second_avg / 1_000_000.0);
            println!("{}Progresso: {:.6}%", colors::CYAN, progress_percent);
            println!("{}Tempo restante estimado: {}", colors::CYAN, remaining_time);
            println!("{}Última chave: {}{}", colors::CYAN, last_key_hex, colors::RESET);
        }
    });
    
    // Create worker chunks
    let mut worker_handles = vec![];
    
    for i in 0..num_workers {
        let worker_start = &random_start + (&chunk_size * i);
        let mut worker_end = &worker_start + &chunk_size;
        
        // Make sure we don't exceed the overall max
        if worker_end > max_key || i == num_workers - 1 {
            worker_end = max_key.clone();
        }
        
        // Handle wrap-around if we exceed max_key
        let worker_start = if worker_start > max_key {
            // Wrap around to min_key plus the remainder
            let excess = &worker_start - &max_key - 1u32;
            &min_key + excess
        } else {
            worker_start
        };
        
        // Create clones of the Arc variables for each worker
        let found_match_clone = Arc::clone(&found_match);
        let found_key_clone = Arc::clone(&found_key);
        let found_hash160_clone = Arc::clone(&found_hash160);
        let total_iterations_clone = Arc::clone(&total_iterations);
        let last_key_checked_clone = Arc::clone(&last_key_checked);
        let target_hash160_clone = target_hash160.to_vec();
        let min_key_clone = min_key.clone();
        
        // Create the worker
        worker_handles.push(thread::spawn(move || {
            // Initial setup
            let mut current_key = worker_start;
            let one = BigUint::one();
            let mut worker_iterations: u64 = 0;
            let mut key_batch = KeyBatch::new(BATCH_SIZE);
            
            // Main loop for this worker
            'main_loop: while &current_key <= &worker_end {
                // Process a batch of keys
                // Clear the batch for reuse
                key_batch.clear();
                
                // Generate a batch of private keys
                for _ in 0..BATCH_SIZE {
                    // Check if we're done or should wrap around
                    if current_key > worker_end {
                        if worker_iterations % 1000 == 0 {
                            last_key_checked_clone.lock().unwrap().clone_from(&current_key);
                        }
                        current_key = min_key_clone.clone();
                    }
                    
                    // Check periodically for match from other workers
                    if key_batch.private_keys.len() % 64 == 0 && 
                       found_match_clone.load(AtomicOrdering::Relaxed) {
                        break 'main_loop;
                    }
                    
                    // Add key to batch
                    let private_key_bytes = pad_private_key(&current_key.to_bytes_be(), 32);
                    key_batch.private_keys.push(private_key_bytes);
                    
                    // Move to next key
                    current_key += &one;
                }
                
                let batch_size = key_batch.private_keys.len();
                
                // Process the batch to generate hash160s
                key_batch.hash160s = key_batch.private_keys.par_iter()
                    .filter_map(|private_key_bytes| {
                        match private_key_to_hash160(private_key_bytes) {
                            Ok(hash160) => Some(hash160),
                            Err(_) => None,
                        }
                    })
                    .collect();
                
                // Check if we have enough hash160s
                if key_batch.hash160s.len() != batch_size {
                    // Some error occurred during hash160 generation
                    eprintln!("{}Warning: Some keys could not be processed{}", colors::YELLOW, colors::RESET);
                }
                
                // Busca paralela pelo hash160 alvo
                let match_index = key_batch.hash160s.par_iter()
                    .position_any(|hash160| bytes_equal(hash160, &target_hash160_clone));
                
                // Se encontrou um match
                if let Some(index) = match_index {
                    // We found a match!
                    if !found_match_clone.load(AtomicOrdering::Relaxed) {
                        found_match_clone.store(true, AtomicOrdering::Relaxed);
                        
                        // Store the found key and hash160
                        let mut found_key_guard = found_key_clone.lock().unwrap();
                        *found_key_guard = key_batch.private_keys[index].clone();
                        
                        let mut found_hash160_guard = found_hash160_clone.lock().unwrap();
                        *found_hash160_guard = key_batch.hash160s[index].clone();
                    }
                    break 'main_loop;
                }
                
                // Atualizar contadores
                worker_iterations += batch_size as u64;
                total_iterations_clone.fetch_add(batch_size as u64, AtomicOrdering::Relaxed);
                
                // Periodicamente atualizar a última chave verificada
                if worker_iterations % 1000 == 0 {
                    let mut last_key = last_key_checked_clone.lock().unwrap();
                    *last_key = current_key.clone();
                }
                
                // Verificar periodicamente se outros workers encontraram um match
                if found_match_clone.load(AtomicOrdering::Relaxed) {
                    break 'main_loop;
                }
            }
        }));
    }
    
    // Wait for all workers to finish
    for handle in worker_handles {
        let _ = handle.join();
    }
    
    // Report results
    if found_match.load(AtomicOrdering::Relaxed) {
        let found_key_guard = found_key.lock().unwrap();
        let private_key_hex = hex::encode(&*found_key_guard);
        println!("\n{}MATCH FOUND!{}", colors::BOLD_GREEN, colors::RESET);
        println!("{}Private Key: {}{}{}", colors::GREEN, colors::BOLD_GREEN, private_key_hex, colors::RESET);
        
        let found_hash160_guard = found_hash160.lock().unwrap();
        let hash160_hex = hex::encode(&*found_hash160_guard);
        println!("{}Hash160: {}{}{}", colors::GREEN, colors::BOLD_GREEN, hash160_hex, colors::RESET);
        
        // Calcular a chave privada em formato decimal
        let found_key_bigint = BigUint::from_bytes_be(&*found_key_guard);
        let found_key_decimal = found_key_bigint.to_string();
        
        // Gerar chave WIF usando a biblioteca bitcoin
        let wif = match private_key_to_wif(&*found_key_guard) {
            Ok(wif) => wif,
            Err(_) => "Erro ao gerar WIF".to_string(),
        };
        
        // Gerar endereço Bitcoin a partir da chave privada
        let address = match private_key_to_p2pkh_address(&*found_key_guard) {
            Ok(addr) => addr,
            Err(_) => "Erro ao gerar endereço".to_string(),
        };
        
        // Write the private key to a file
        let filename = format!("bitcoin_puzzle_solution_{}.txt", &hash160_hex[..8]);
        let content = format!(
            "\n=== BITCOIN PUZZLE SOLUTION ===\n\n\
            Private Key (hex): {}\n\
            Private Key (decimal): {}\n\
            WIF: {}\n\
            Address: {}\n\
            Hash160: {}\n\n\
            Found at: {}\n\n\
            Congratulations! You've found a Bitcoin puzzle solution!\n\
            Please verify this solution and claim your reward.\n\
            https://privatekeys.pw/puzzles/bitcoin-puzzle-tx\n",
            private_key_hex, 
            found_key_decimal,
            wif,
            address,
            hash160_hex, 
            Local::now().format("%Y-%m-%dT%H:%M:%S%z")
        );
        
        match File::create(&filename).and_then(|mut file| file.write_all(content.as_bytes())) {
            Ok(_) => {
                println!("{}Solução salva no arquivo: {}{}{}", 
                         colors::GREEN, colors::BOLD_GREEN, filename, colors::RESET);
                println!("{}Chave WIF: {}{}{}", 
                         colors::GREEN, colors::BOLD_GREEN, wif, colors::RESET);
                println!("{}Endereço Bitcoin: {}{}{}", 
                         colors::GREEN, colors::BOLD_GREEN, address, colors::RESET);
            },
            Err(err) => {
                eprintln!("{}Erro ao salvar resultado no arquivo: {}{}", colors::RED, err, colors::RESET);
            }
        }
    } else {
        println!("\n{}Nenhum resultado encontrado após verificar aproximadamente {} chaves.{}", 
                 colors::YELLOW, total_iterations.load(AtomicOrdering::Relaxed), colors::RESET);
    }
} 