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

// Frequência de atualização das estatísticas (em segundos)
const STATS_UPDATE_INTERVAL: u64 = 5;

// Compare two byte slices for equality - versão otimizada
fn bytes_equal(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    
    // Usando iteração e verificação byte-a-byte
    a.iter().zip(b.iter()).all(|(x, y)| x == y)
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
                let padded_key = pad_private_key(&key.to_bytes_be(), 32); // Usar 32 bytes para chave privada
                
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
        let padded_key = pad_private_key(&found_key.to_bytes_be(), 32); // Usar 32 bytes para chave privada
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

// Nova função otimizada que utiliza chunks pré-calculados e monitoramento de performance
pub fn search_for_private_key_optimized(chunks: &[(BigUint, BigUint)], target_hash160: &[u8], batch_size: usize) {
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
    
    // Número de chunks para processamento paralelo
    println!("{}Usando {} chunks para processamento paralelo{}", 
            colors::GREEN, chunks.len(), colors::RESET);
    
    // Processamento paralelo dos chunks predefinidos com melhor balanceamento
    chunks.par_iter().for_each(|(chunk_min, chunk_max)| {
        // Se já encontrou a chave, não iniciar novos chunks
        if found.load(AtomicOrdering::Relaxed) {
            return;
        }
        
        let mut current_key = chunk_min.clone();
        let mut batch = Vec::with_capacity(batch_size);
        
        while &current_key <= chunk_max {
            // Preencher o batch com tamanho otimizado
            batch.clear();
            for _ in 0..batch_size {
                if &current_key > chunk_max {
                    break;
                }
                
                batch.push(current_key.clone());
                current_key += 1u64;
            }
            
            let batch_len = batch.len();
            
            // Processar o batch
            for key in &batch {
                // Se já encontrou a chave, sair do loop
                if found.load(AtomicOrdering::Relaxed) {
                    return;
                }
                
                // Conversão da chave para hash160
                let padded_key = pad_private_key(&key.to_bytes_be(), 32); // Usar 32 bytes para chave privada
                
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
            
            // Incrementar contador global de chaves verificadas
            performance::increment_keys_checked(batch_len);
            
            // Verificar se a busca já terminou por outro thread
            if found.load(AtomicOrdering::Relaxed) {
                return;
            }
        }
    });
    
    // Aguardar thread de estatísticas
    let _ = stats_thread.join();
    
    // Verificar se encontrou
    if found.load(AtomicOrdering::Relaxed) {
        let found_key = found_key.lock().unwrap();
        let padded_key = pad_private_key(&found_key.to_bytes_be(), 32); // Usar 32 bytes para chave privada
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