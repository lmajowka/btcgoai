#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod models;
mod bitcoin;
mod search;
mod colors;
mod data;
mod bitcoin_puzzle;
mod bitcoin_puzzle_test;

use std::io::{self, BufRead, Write};
use num_bigint::BigUint;
use num_traits::ToPrimitive;
use hex;

fn main() {
    // Configurar thread pool para o máximo de performance
    let num_threads = num_cpus::get() * 2;
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build_global()
        .unwrap();
        
    println!("{}Bitcoin Private Key Finder (Rust) - Puzzle TX Edition{}", colors::BOLD_GREEN, colors::RESET);
    println!("{}Initializing with {} threads{}\n", colors::CYAN, num_threads, colors::RESET);

    // Menu principal com 3 opções claras
    println!("{}Modos disponíveis:{}", colors::BOLD_YELLOW, colors::RESET);
    println!("{}1. Modo Normal - Tenta resolver puzzles reais do Bitcoin Puzzle TX{}", colors::CYAN, colors::RESET);
    println!("{}2. Modo Treinamento - Usa puzzles pequenos (5-22 bits) com chaves conhecidas{}", colors::CYAN, colors::RESET);
    println!("{}3. Modo Teste de Ranges - Verifica intervalos dos puzzles não resolvidos sem busca efetiva{}", colors::CYAN, colors::RESET);
    
    print!("\n{}Digite o número do modo desejado (1-3): {}", colors::BOLD_CYAN, colors::RESET);
    io::stdout().flush().unwrap();
    
    let stdin = io::stdin();
    let mode_choice_str = stdin.lock().lines().next().unwrap().unwrap();
    let mode_choice: usize = match mode_choice_str.trim().parse() {
        Ok(num) if num >= 1 && num <= 3 => num,
        _ => {
            println!("{}Escolha inválida. Iniciando em modo normal (1).{}", 
                     colors::YELLOW, colors::RESET);
            1
        }
    };
    
    match mode_choice {
        1 => run_normal_mode(num_threads),
        2 => run_training_mode(num_threads),
        3 => run_range_test_mode(num_threads),
        _ => run_normal_mode(num_threads), // Fallback para modo normal
    }
}

fn run_training_mode(num_threads: usize) {
    println!("{}Modo de TREINAMENTO ativado - Usando puzzles pequenos para verificação{}", colors::BOLD_GREEN, colors::RESET);
    
    // Carregar os puzzles de teste
    let mut test_puzzles = bitcoin_puzzle_test::get_test_puzzles();
    match bitcoin_puzzle_test::convert_addresses_to_hash160(&mut test_puzzles) {
        Ok(_) => (),
        Err(err) => {
            println!("{}Erro ao converter endereços para hash160: {}{}", colors::RED, err, colors::RESET);
            return;
        }
    }
    println!("{}Carregados {} puzzles de treinamento{}", colors::GREEN, test_puzzles.len(), colors::RESET);

    // Prompt user para escolher o puzzle
    println!("\n{}Puzzles de treinamento (baixa dificuldade para verificação):{}", colors::BOLD_YELLOW, colors::RESET);
    println!("{}-------------------------------------------------{}", colors::CYAN, colors::RESET);
    
    println!("{}   # | Puzzle # | Endereço                       | Bits | Chave Privada{}", 
             colors::CYAN, colors::RESET);
    println!("{}---------------------------------------------------------------------------------{}", 
             colors::CYAN, colors::RESET);
    
    for (i, puzzle) in test_puzzles.iter().enumerate() {
        println!("{}{:4} | {:8} | {:29} | {:4} | {}...{}{}",
                colors::CYAN, i+1, puzzle.puzzle_number, puzzle.address, 
                puzzle.bits, &puzzle.private_key[0..10], &puzzle.private_key[54..64], colors::RESET);
    }
    
    // Prompt user para escolher o puzzle
    print!("\n{}Escolha o número do puzzle para testar (1-{}): {}", 
           colors::CYAN, test_puzzles.len(), colors::RESET);
    io::stdout().flush().unwrap();
    
    let stdin = io::stdin();
    let puzzle_choice_str = stdin.lock().lines().next().unwrap().unwrap();
    let puzzle_choice: usize = match puzzle_choice_str.trim().parse() {
        Ok(num) if num >= 1 && num <= test_puzzles.len() => num,
        _ => {
            println!("{}Escolha inválida. Por favor, escolha um número entre 1 e {}.{}", 
                     colors::RED, test_puzzles.len(), colors::RESET);
            return;
        }
    };

    // Obter o puzzle selecionado
    let selected_puzzle = &test_puzzles[puzzle_choice - 1];
    bitcoin_puzzle_test::display_test_puzzle_info(selected_puzzle);
    
    // Converter o puzzle para um intervalo de busca
    let selected_range = bitcoin_puzzle_test::puzzle_to_range(selected_puzzle);

    let target_hash160_hex = hex::encode(&selected_puzzle.hash160);
    println!("{}Hash160 selecionado: {}{}{}", colors::YELLOW, colors::BOLD_YELLOW, target_hash160_hex, colors::RESET);
    println!("{}Intervalo: min={}{}{}, max={}{}{}", 
             colors::YELLOW, colors::BOLD_CYAN, selected_range.min, colors::RESET, 
             colors::BOLD_CYAN, selected_range.max, colors::RESET);
    
    // Calcular estimativa de tempo com base na dificuldade
    let bits = selected_puzzle.bits;
    let keyspace_size = BigUint::from(2u32).pow(bits);
    
    // Velocidade estimada em chaves por segundo (baseado em testes anteriores)
    let est_keys_per_sec = 1_000_000 * num_threads as u64; // Estimativa de 1M de chaves por thread por segundo
    
    // Calcular tempo estimado
    let est_seconds = keyspace_size.to_f64().unwrap() / est_keys_per_sec as f64;
    
    println!("\n{}Estimativa de tempo para busca completa (velocidade estimada: {} M chaves/s):{}", 
             colors::YELLOW, est_keys_per_sec / 1_000_000, colors::RESET);
    
    if est_seconds > 3600.0 {
        let est_hours = est_seconds / 3600.0;
        println!("{}Aproximadamente {:.2} horas{}", colors::YELLOW, est_hours, colors::RESET);
    } else if est_seconds > 60.0 {
        let est_minutes = est_seconds / 60.0;
        println!("{}Aproximadamente {:.2} minutos{}", colors::GREEN, est_minutes, colors::RESET);
    } else {
        println!("{}Aproximadamente {:.2} segundos{}", colors::GREEN, est_seconds, colors::RESET);
    }
    
    // Probabilidade de sucesso
    println!("\n{}Probabilidade de encontrar a chave:{}", colors::YELLOW, colors::RESET);
    println!("{}1 em {} (chance de {:.10}%){}", 
             colors::CYAN, keyspace_size.to_string(), 100.0 / keyspace_size.to_f64().unwrap(), colors::RESET);
    
    // Perguntar se o usuário deseja continuar
    print!("\n{}Deseja continuar a busca? (S/N): {}", colors::CYAN, colors::RESET);
    io::stdout().flush().unwrap();
    
    let confirm_str = stdin.lock().lines().next().unwrap().unwrap();
    if !confirm_str.trim().eq_ignore_ascii_case("s") && 
       !confirm_str.trim().eq_ignore_ascii_case("sim") &&
       !confirm_str.trim().eq_ignore_ascii_case("y") &&
       !confirm_str.trim().eq_ignore_ascii_case("yes") {
        println!("{}Busca cancelada pelo usuário.{}", colors::YELLOW, colors::RESET);
        return;
    }

    // Converter strings hex para BigUint
    let min_key = BigUint::parse_bytes(&selected_range.min[2..].as_bytes(), 16).unwrap(); // Remover prefixo 0x
    let max_key = BigUint::parse_bytes(&selected_range.max[2..].as_bytes(), 16).unwrap(); // Remover prefixo 0x

    search::search_for_private_key(&min_key, &max_key, &selected_puzzle.hash160);
}

fn run_range_test_mode(num_threads: usize) {
    println!("{}Modo de TESTE DE RANGES ativado - Verificando intervalos dos puzzles não resolvidos{}", colors::BOLD_GREEN, colors::RESET);
    
    // Carregar os puzzles Bitcoin não resolvidos
    let mut puzzles = bitcoin_puzzle::get_unsolved_puzzles();
    match bitcoin_puzzle::convert_addresses_to_hash160(&mut puzzles) {
        Ok(_) => (),
        Err(err) => {
            println!("{}Erro ao converter endereços para hash160: {}{}", colors::RED, err, colors::RESET);
            return;
        }
    }
    println!("{}Carregados {} puzzles não resolvidos{}", colors::GREEN, puzzles.len(), colors::RESET);

    // Prompt user para escolher o puzzle
    println!("\n{}Bitcoin Puzzle TX Challenge - Teste de Ranges:{}", colors::BOLD_YELLOW, colors::RESET);
    println!("{}-------------------------------------------------{}", colors::CYAN, colors::RESET);
    
    // Organizar puzzles por dificuldade (bits)
    let mut puzzles_by_difficulty: Vec<Vec<&bitcoin_puzzle::BitcoinPuzzle>> = vec![Vec::new(); 161]; // Índices de 0 a 160 para cobrir todos os puzzles
    for puzzle in &puzzles {
        puzzles_by_difficulty[puzzle.bits as usize].push(puzzle);
    }
    
    let mut display_index = 1;
    println!("{}   # | Puzzle # | Endereço                       | Recompensa | Bits (Dificuldade){}", 
             colors::CYAN, colors::RESET);
    println!("{}-------------------------------------------------------------------{}", 
             colors::CYAN, colors::RESET);
    
    for bits in 71..=160 {
        for puzzle in &puzzles_by_difficulty[bits as usize] {
            println!("{}{:4} | {:8} | {:29} | {:9.1} BTC | {:3} bits{}",
                    colors::CYAN, display_index, puzzle.puzzle_number, puzzle.address, 
                    puzzle.reward, puzzle.bits, colors::RESET);
            display_index += 1;
        }
    }
    
    // Prompt user para escolher o puzzle
    print!("\n{}Escolha o número do puzzle para verificar o range (1-{}): {}", 
           colors::CYAN, puzzles.len(), colors::RESET);
    io::stdout().flush().unwrap();
    
    let stdin = io::stdin();
    let puzzle_choice_str = stdin.lock().lines().next().unwrap().unwrap();
    let puzzle_choice: usize = match puzzle_choice_str.trim().parse() {
        Ok(num) if num >= 1 && num <= puzzles.len() => num,
        _ => {
            println!("{}Escolha inválida. Por favor, escolha um número entre 1 e {}.{}", 
                     colors::RED, puzzles.len(), colors::RESET);
            return;
        }
    };

    // Obter o puzzle selecionado
    let selected_puzzle = &puzzles[puzzle_choice - 1];
    bitcoin_puzzle::display_puzzle_info(selected_puzzle);
    
    // Converter o puzzle para um intervalo de busca
    let selected_range = bitcoin_puzzle::puzzle_to_range(selected_puzzle);

    let target_hash160_hex = hex::encode(&selected_puzzle.hash160);
    println!("{}Hash160 selecionado: {}{}{}", colors::YELLOW, colors::BOLD_YELLOW, target_hash160_hex, colors::RESET);
    println!("{}Intervalo: min={}{}{}, max={}{}{}", 
             colors::YELLOW, colors::BOLD_CYAN, selected_range.min, colors::RESET, 
             colors::BOLD_CYAN, selected_range.max, colors::RESET);
    
    // Calcular estimativa de tempo com base na dificuldade
    let bits = selected_puzzle.bits;
    let keyspace_size = BigUint::from(2u32).pow(bits);
    
    // Velocidade estimada em chaves por segundo (baseado em testes anteriores)
    let est_keys_per_sec = 1_000_000 * num_threads as u64; // Estimativa de 1M de chaves por thread por segundo
    
    // Calcular tempo estimado
    let est_seconds = keyspace_size.to_f64().unwrap() / est_keys_per_sec as f64;
    let est_days = est_seconds / (24.0 * 60.0 * 60.0);
    let est_years = est_days / 365.25;
    
    println!("\n{}Estimativa de tempo para busca completa (velocidade estimada: {} M chaves/s):{}", 
             colors::YELLOW, est_keys_per_sec / 1_000_000, colors::RESET);
    
    if est_years > 1.0 {
        println!("{}Aproximadamente {:.2} anos{}", colors::RED, est_years, colors::RESET);
    } else if est_days > 1.0 {
        println!("{}Aproximadamente {:.2} dias{}", colors::YELLOW, est_days, colors::RESET);
    } else {
        let est_hours = est_seconds / 3600.0;
        println!("{}Aproximadamente {:.2} horas{}", colors::GREEN, est_hours, colors::RESET);
    }
    
    // Probabilidade de sucesso
    println!("\n{}Probabilidade de encontrar a chave:{}", colors::YELLOW, colors::RESET);
    println!("{}1 em {} (chance de {:.10}%){}", 
             colors::CYAN, keyspace_size.to_string(), 100.0 / keyspace_size.to_f64().unwrap(), colors::RESET);
    
    // Calcular percentual aproximado do espaço de busca
    let full_keyspace_size = BigUint::from(2u32).pow(256); // Espaço total da chave privada Bitcoin (256 bits)
    let search_percentage = (keyspace_size.to_f64().unwrap() / full_keyspace_size.to_f64().unwrap()) * 100.0;
    
    println!("{}Este puzzle representa aproximadamente {:.20}% do espaço total de chaves Bitcoin{}", 
             colors::CYAN, search_percentage, colors::RESET);
    
    // Informações adicionais sobre o intervalo de busca
    let min_key = BigUint::parse_bytes(&selected_range.min[2..].as_bytes(), 16).unwrap(); // Remover prefixo 0x
    let max_key = BigUint::parse_bytes(&selected_range.max[2..].as_bytes(), 16).unwrap(); // Remover prefixo 0x
    let range_size = &max_key - &min_key;
    
    println!("\n{}Detalhes do intervalo de busca:{}", colors::BOLD_YELLOW, colors::RESET);
    println!("{}Início (min): {}{}", colors::CYAN, selected_range.min, colors::RESET);
    println!("{}Fim (max): {}{}", colors::CYAN, selected_range.max, colors::RESET);
    println!("{}Tamanho do intervalo: {}{}", colors::CYAN, range_size.to_string(), colors::RESET);
    
    println!("\n{}OBSERVAÇÃO: Este modo apenas verifica o intervalo sem iniciar a busca real.{}", 
             colors::BOLD_YELLOW, colors::RESET);
    println!("{}Para iniciar uma busca real neste puzzle, use o Modo Normal (opção 1).{}", 
             colors::YELLOW, colors::RESET);
}

fn run_normal_mode(num_threads: usize) {
    // Carregar os puzzles Bitcoin não resolvidos
    let mut puzzles = bitcoin_puzzle::get_unsolved_puzzles();
    match bitcoin_puzzle::convert_addresses_to_hash160(&mut puzzles) {
        Ok(_) => (),
        Err(err) => {
            println!("{}Erro ao converter endereços para hash160: {}{}", colors::RED, err, colors::RESET);
            return;
        }
    }
    println!("{}Carregados {} puzzles não resolvidos{}", colors::GREEN, puzzles.len(), colors::RESET);

    // Prompt user para escolher o puzzle
    println!("\n{}Bitcoin Puzzle TX Challenge - Puzzles não resolvidos:{}", colors::BOLD_YELLOW, colors::RESET);
    println!("{}-------------------------------------------------{}", colors::CYAN, colors::RESET);
    
    // Organizar puzzles por dificuldade (bits)
    let mut puzzles_by_difficulty: Vec<Vec<&bitcoin_puzzle::BitcoinPuzzle>> = vec![Vec::new(); 161]; // Índices de 0 a 160 para cobrir todos os puzzles
    for puzzle in &puzzles {
        puzzles_by_difficulty[puzzle.bits as usize].push(puzzle);
    }
    
    let mut display_index = 1;
    println!("{}   # | Puzzle # | Endereço                       | Recompensa | Bits (Dificuldade){}", 
             colors::CYAN, colors::RESET);
    println!("{}-------------------------------------------------------------------{}", 
             colors::CYAN, colors::RESET);
    
    for bits in 71..=160 {
        for puzzle in &puzzles_by_difficulty[bits as usize] {
            println!("{}{:4} | {:8} | {:29} | {:9.1} BTC | {:3} bits{}",
                    colors::CYAN, display_index, puzzle.puzzle_number, puzzle.address, 
                    puzzle.reward, puzzle.bits, colors::RESET);
            display_index += 1;
        }
    }
    
    // Prompt user para escolher o puzzle
    print!("\n{}Escolha o número do puzzle para tentar resolver (1-{}): {}", 
           colors::CYAN, puzzles.len(), colors::RESET);
    io::stdout().flush().unwrap();
    
    let stdin = io::stdin();
    let puzzle_choice_str = stdin.lock().lines().next().unwrap().unwrap();
    let puzzle_choice: usize = match puzzle_choice_str.trim().parse() {
        Ok(num) if num >= 1 && num <= puzzles.len() => num,
        _ => {
            println!("{}Escolha inválida. Por favor, escolha um número entre 1 e {}.{}", 
                     colors::RED, puzzles.len(), colors::RESET);
            return;
        }
    };

    // Obter o puzzle selecionado
    let selected_puzzle = &puzzles[puzzle_choice - 1];
    bitcoin_puzzle::display_puzzle_info(selected_puzzle);
    
    // Converter o puzzle para um intervalo de busca
    let selected_range = bitcoin_puzzle::puzzle_to_range(selected_puzzle);

    let target_hash160_hex = hex::encode(&selected_puzzle.hash160);
    println!("{}Hash160 selecionado: {}{}{}", colors::YELLOW, colors::BOLD_YELLOW, target_hash160_hex, colors::RESET);
    println!("{}Intervalo: min={}{}{}, max={}{}{}", 
             colors::YELLOW, colors::BOLD_CYAN, selected_range.min, colors::RESET, 
             colors::BOLD_CYAN, selected_range.max, colors::RESET);
    
    // Calcular estimativa de tempo com base na dificuldade
    let bits = selected_puzzle.bits;
    let keyspace_size = BigUint::from(2u32).pow(bits);
    
    // Velocidade estimada em chaves por segundo (baseado em testes anteriores)
    let est_keys_per_sec = 1_000_000 * num_threads as u64; // Estimativa de 1M de chaves por thread por segundo
    
    // Calcular tempo estimado
    let est_seconds = keyspace_size.to_f64().unwrap() / est_keys_per_sec as f64;
    let est_days = est_seconds / (24.0 * 60.0 * 60.0);
    let est_years = est_days / 365.25;
    
    println!("\n{}Estimativa de tempo para busca completa (velocidade estimada: {} M chaves/s):{}", 
             colors::YELLOW, est_keys_per_sec / 1_000_000, colors::RESET);
    
    if est_years > 1.0 {
        println!("{}Aproximadamente {:.2} anos{}", colors::RED, est_years, colors::RESET);
    } else if est_days > 1.0 {
        println!("{}Aproximadamente {:.2} dias{}", colors::YELLOW, est_days, colors::RESET);
    } else {
        let est_hours = est_seconds / 3600.0;
        println!("{}Aproximadamente {:.2} horas{}", colors::GREEN, est_hours, colors::RESET);
    }
    
    // Probabilidade de sucesso
    println!("\n{}Probabilidade de encontrar a chave:{}", colors::YELLOW, colors::RESET);
    println!("{}1 em {} (chance de {:.10}%){}", 
             colors::CYAN, keyspace_size.to_string(), 100.0 / keyspace_size.to_f64().unwrap(), colors::RESET);
    
    // Perguntar se o usuário deseja continuar
    print!("\n{}Deseja continuar a busca? (S/N): {}", colors::CYAN, colors::RESET);
    io::stdout().flush().unwrap();
    
    let confirm_str = stdin.lock().lines().next().unwrap().unwrap();
    if !confirm_str.trim().eq_ignore_ascii_case("s") && 
       !confirm_str.trim().eq_ignore_ascii_case("sim") &&
       !confirm_str.trim().eq_ignore_ascii_case("y") &&
       !confirm_str.trim().eq_ignore_ascii_case("yes") {
        println!("{}Busca cancelada pelo usuário.{}", colors::YELLOW, colors::RESET);
        return;
    }

    // Converter strings hex para BigUint
    let min_key = BigUint::parse_bytes(&selected_range.min[2..].as_bytes(), 16).unwrap(); // Remover prefixo 0x
    let max_key = BigUint::parse_bytes(&selected_range.max[2..].as_bytes(), 16).unwrap(); // Remover prefixo 0x

    search::search_for_private_key(&min_key, &max_key, &selected_puzzle.hash160);
} 