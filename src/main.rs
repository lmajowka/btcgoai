#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

mod models;
mod bitcoin;
mod search;
mod colors;
mod data;
mod bitcoin_puzzle;
mod bitcoin_puzzle_test;
mod performance;
#[cfg(feature = "opencl")]
mod gpu;

use std::io::{self, BufRead, Write};
use std::fs::File;
use num_bigint::BigUint;
use num_traits::ToPrimitive;
use hex;
use rand::{self, Rng};
use chrono::Local;

#[cfg(feature = "opencl")]
use crate::gpu::check_opencl_availability;

#[cfg(feature = "opencl")]
type OptionalGpuSearcher = Option<gpu::GpuSearcher>;

#[cfg(not(feature = "opencl"))]
type OptionalGpuSearcher = Option<()>; // Using unit type when GPU support is not compiled

fn print_header() {
    println!("{}Bitcoin Private Key Finder (Rust) - v0.1.0{}", colors::BOLD_GREEN, colors::RESET);
    println!("{}Compiled with GPU support: {}{}", colors::CYAN, 
             if cfg!(feature = "opencl") { "Yes" } else { "No" }, 
             colors::RESET);
}

fn print_system_info(resources: &performance::SystemResources) {
    // Print system information
    println!("{}System Information:{}", colors::BOLD_YELLOW, colors::RESET);
    println!("{}CPU: {}{}", colors::CYAN, resources.cpu_brand, colors::RESET);
    println!("{}Physical cores: {}, Logical threads: {}{}", 
             colors::CYAN, resources.cpu_count, resources.thread_count, colors::RESET);
    println!("{}Total memory: {:.2} GB, Available: {:.2} GB{}", 
             colors::CYAN, 
             resources.total_memory as f64 / (1024.0 * 1024.0 * 1024.0),
             resources.available_memory as f64 / (1024.0 * 1024.0 * 1024.0),
             colors::RESET);
    
    // SIMD instructions
    let simd_info = format!("{}{}{}{}",
                           if resources.has_avx2 { "AVX2 " } else { "" },
                           if resources.has_avx { "AVX " } else { "" },
                           if resources.has_sse { "SSE " } else { "" },
                           if !resources.has_avx2 && !resources.has_avx && !resources.has_sse { "None" } else { "" });
    println!("{}SIMD instructions: {}{}", colors::CYAN, simd_info, colors::RESET);
}

fn main() {
    // Configuração para otimizações de tempo de execução
    // Configure mimalloc como alocador global
    #[cfg(feature = "mimalloc")]
    {
        // Nota: O MiMalloc 0.1.34 não suporta mais estas funções
        // mimalloc::MiMalloc::set_as_global();
        // mimalloc::option::set_option(mimalloc::Option::ShowStats, true);
        // mimalloc::option::set_option(mimalloc::Option::ShowErrors, true);
        println!("{}Usando MiMalloc como alocador de memória otimizado{}", colors::GREEN, colors::RESET);
    }
    
    // Ajustar parâmetros iniciais de busca
    let mut params = performance::SearchParameters::default();
    params.threads = num_cpus::get();
    
    // Configurar o modo de verificação de OpenCL
    let _check_warnings: bool = true;
    
    print_header();
    
    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let mut resource_usage: u8 = 75; // Default: use 75% of resources
    
    // Parse arguments
    for i in 1..args.len() {
        if args[i] == "--usage" && i + 1 < args.len() {
            if let Ok(usage) = args[i+1].parse::<u8>() {
                if usage >= 10 && usage <= 100 {
                    resource_usage = usage;
                } else {
                    println!("{}Invalid resource usage value (10-100). Using default: 75%{}", colors::YELLOW, colors::RESET);
                }
            }
        } else if args[i] == "--help" || args[i] == "-h" {
            println!("\n{}Usage:{}", colors::BOLD_YELLOW, colors::RESET);
            println!("  btcrustai [options]");
            println!("\n{}Options:{}", colors::BOLD_YELLOW, colors::RESET);
            println!("  --usage <percent>   Set resource usage percentage (10-100)");
            println!("  --help, -h          Display this help message");
            return;
        }
    }
    
    // Detect system resources
    let resources = performance::detect_system_resources();
    print_system_info(&resources);
    
    // Procurar por dispositivos GPU
    println!("\n{}Verificando dispositivos GPU disponíveis...{}", colors::BOLD_YELLOW, colors::RESET);

    #[cfg(feature = "opencl")]
    let opencl_available = check_opencl_availability();

    #[cfg(feature = "opencl")]
    let gpu_searcher = if opencl_available {
        match gpu::GpuSearcher::new() {
            Ok(searcher) => {
                let devices = searcher.list_devices();
                if devices.is_empty() {
                    println!("{}Nenhum dispositivo OpenCL/GPU encontrado.{}", colors::YELLOW, colors::RESET);
                    None
                } else {
                    println!("{}Dispositivos GPU encontrados:{}", colors::GREEN, colors::RESET);
                    for (i, (name, _)) in devices.iter().enumerate() {
                        println!("{}{:2}. {}{}", colors::CYAN, i+1, name, colors::RESET);
                    }
                    Some(searcher)
                }
            },
            Err(e) => {
                println!("{}Erro ao inicializar GPU: {}{}", colors::YELLOW, e, colors::RESET);
                None
            }
        }
    } else {
        println!("{}OpenCL não encontrado ou não disponível no sistema{}", colors::YELLOW, colors::RESET);
        None
    };
    
    #[cfg(not(feature = "opencl"))]
    {
        println!("{}Suporte a GPU não compilado nesta versão{}", colors::YELLOW, colors::RESET);
        let gpu_searcher: OptionalGpuSearcher = None;
    }
    
    // Perguntar se o usuário quer usar GPU ou CPU
    #[cfg(feature = "opencl")]
    let use_gpu = if gpu_searcher.is_some() {
        print!("\n{}Deseja utilizar a GPU para aceleração? (S/N): {}", colors::BOLD_CYAN, colors::RESET);
        io::stdout().flush().unwrap();
        
        let stdin = io::stdin();
        let gpu_choice = stdin.lock().lines().next().unwrap().unwrap();
        gpu_choice.trim().eq_ignore_ascii_case("s") || 
        gpu_choice.trim().eq_ignore_ascii_case("sim") ||
        gpu_choice.trim().eq_ignore_ascii_case("y") ||
        gpu_choice.trim().eq_ignore_ascii_case("yes")
    } else {
        false
    };

    #[cfg(not(feature = "opencl"))]
    let _use_gpu = false;

    // Se o usuário escolheu GPU, selecionar o dispositivo
    #[cfg(feature = "opencl")]
    let gpu_searcher = if use_gpu {
        let mut searcher = gpu_searcher.unwrap();
        let devices = searcher.list_devices();
        
        // Perguntar qual dispositivo usar se houver mais de um
        if devices.len() > 1 {
            print!("\n{}Escolha o dispositivo GPU (1-{}): {}", 
                  colors::BOLD_CYAN, devices.len(), colors::RESET);
            io::stdout().flush().unwrap();
            
            let stdin = io::stdin();
            let device_choice_str = stdin.lock().lines().next().unwrap().unwrap();
            let device_idx: usize = match device_choice_str.trim().parse::<usize>() {
                Ok(num) if num >= 1 && num <= devices.len() => num - 1,
                _ => {
                    println!("{}Escolha inválida. Usando o primeiro dispositivo.{}", 
                            colors::YELLOW, colors::RESET);
                    0
                }
            };
            
            if let Err(e) = searcher.select_device(device_idx) {
                println!("{}Erro ao selecionar dispositivo: {}{}", colors::RED, e, colors::RESET);
                println!("{}Revertendo para CPU.{}", colors::YELLOW, colors::RESET);
                None
            } else {
                println!("{}Dispositivo selecionado: {}{}", 
                        colors::GREEN, devices[device_idx].0, colors::RESET);
                
                // Inicializar o programa OpenCL
                if let Err(e) = searcher.initialize_program() {
                    println!("{}Erro ao inicializar programa OpenCL: {}{}", colors::RED, e, colors::RESET);
                    println!("{}Revertendo para CPU.{}", colors::YELLOW, colors::RESET);
                    None
                } else {
                    Some(searcher)
                }
            }
        } else {
            // Se só há um dispositivo, selecionar automaticamente
            if let Err(e) = searcher.select_device(0) {
                println!("{}Erro ao selecionar dispositivo: {}{}", colors::RED, e, colors::RESET);
                println!("{}Revertendo para CPU.{}", colors::YELLOW, colors::RESET);
                None
            } else {
                println!("{}Dispositivo selecionado: {}{}", 
                        colors::GREEN, devices[0].0, colors::RESET);
                
                // Inicializar o programa OpenCL
                if let Err(e) = searcher.initialize_program() {
                    println!("{}Erro ao inicializar programa OpenCL: {}{}", colors::RED, e, colors::RESET);
                    println!("{}Revertendo para CPU.{}", colors::YELLOW, colors::RESET);
                    None
                } else {
                    Some(searcher)
                }
            }
        }
    } else {
        None
    };

    #[cfg(not(feature = "opencl"))]
    let gpu_searcher: OptionalGpuSearcher = None;
    
    // Perguntar quanto dos recursos o usuário deseja utilizar
    // if the usage wasn't passed as a command line argument, ask the user
    if !args.iter().any(|arg| arg == "--usage") {
        print!("\n{}System resource usage percentage (10-100%): {}", colors::BOLD_CYAN, colors::RESET);
        io::stdout().flush().unwrap();
        
        let stdin = io::stdin();
        let resource_usage_str = match stdin.lock().lines().next() {
            Some(Ok(line)) => line,
            _ => {
                println!("{}Failed to read input. Using default 75% of resources.{}", 
                     colors::YELLOW, colors::RESET);
                "75".to_string()
            }
        };
        
        let parsed_usage: u8 = match resource_usage_str.trim().parse() {
            Ok(num) if num >= 10 && num <= 100 => num,
            _ => {
                println!("{}Invalid value. Using 75% of resources.{}", 
                         colors::YELLOW, colors::RESET);
                75
            }
        };
        resource_usage = parsed_usage;
    } else {
        println!("\n{}Using {}% of system resources{}", colors::CYAN, resource_usage, colors::RESET);
    }
    
    // Calcular parâmetros otimizados para a busca
    let params = performance::calculate_optimal_parameters(&resources, resource_usage);
    
    // Configurar o thread pool global com os parâmetros otimizados
    if let Err(e) = performance::configure_thread_pool(&params) {
        println!("{}Erro ao configurar threads: {}{}", colors::RED, e, colors::RESET);
        // Não tentar inicializar novamente se já estiver inicializado
        // Apenas continue usando o pool já configurado
    }
    
    println!("{}Usando {} threads ({} núcleos) e {}% dos recursos do sistema{}", 
             colors::GREEN, params.threads, resources.cpu_count, resource_usage, colors::RESET);
    println!("{}Tamanho de batch otimizado: {} chaves{}", 
             colors::GREEN, params.batch_size, colors::RESET);
    
    // Velocidade estimada baseada nas capacidades do hardware
    let mut est_keys_per_sec = performance::estimate_search_speed(&resources, &params);
    
    // Ajuste da estimativa de velocidade se usando GPU
    if gpu_searcher.is_some() {
        // As GPUs geralmente são 10-100x mais rápidas para este tipo de operação
        est_keys_per_sec *= 20; // Multiplicador conservador
        println!("{}Velocidade estimada com GPU: {:.2} M chaves/s{}\n", 
                colors::GREEN, est_keys_per_sec as f64 / 1_000_000.0, colors::RESET);
    } else {
        println!("{}Velocidade estimada: {:.2} M chaves/s{}\n", 
                colors::GREEN, est_keys_per_sec as f64 / 1_000_000.0, colors::RESET);
    }
    
    // Menu principal com 3 opções claras
    println!("{}Modos disponíveis:{}", colors::BOLD_YELLOW, colors::RESET);
    println!("{}1. Modo Normal - Tenta resolver puzzles reais do Bitcoin Puzzle TX{}", colors::CYAN, colors::RESET);
    println!("{}2. Modo Treinamento - Usa puzzles pequenos (5-22 bits) com chaves conhecidas{}", colors::CYAN, colors::RESET);
    println!("{}3. Modo Teste de Ranges - Verifica intervalos dos puzzles não resolvidos sem busca efetiva{}", colors::CYAN, colors::RESET);
    
    print!("\n{}Digite o número do modo desejado (1-3): {}", colors::BOLD_CYAN, colors::RESET);
    io::stdout().flush().unwrap();
    
    let stdin = io::stdin();
    let mode_choice_str = match stdin.lock().lines().next() {
        Some(Ok(line)) => line,
        _ => {
            println!("{}Falha ao ler entrada. Iniciando em modo normal (1).{}", 
                 colors::YELLOW, colors::RESET);
            "1".to_string()
        }
    };
    
    let mode_choice: usize = match mode_choice_str.trim().parse() {
        Ok(num) if num >= 1 && num <= 3 => num,
        _ => {
            println!("{}Escolha inválida. Iniciando em modo normal (1).{}", 
                     colors::YELLOW, colors::RESET);
            1
        }
    };
    
    match mode_choice {
        1 => run_normal_mode(&params, est_keys_per_sec, gpu_searcher),
        2 => run_training_mode(&params, est_keys_per_sec, gpu_searcher),
        3 => run_range_test_mode(&params, est_keys_per_sec),
        _ => run_normal_mode(&params, est_keys_per_sec, gpu_searcher), // Fallback para modo normal
    }
}

fn run_training_mode(params: &performance::SearchParameters, est_keys_per_sec: u64, gpu_searcher: OptionalGpuSearcher) {
    println!("\n{}Iniciando modo de treinamento...{}", colors::BOLD_GREEN, colors::RESET);
    
    // Mostrar opções de dificuldade
    println!("\n{}Escolha a dificuldade:{}", colors::BOLD_CYAN, colors::RESET);
    println!("1. Muito fácil (5-8 bits)");
    println!("2. Fácil (9-12 bits)");
    println!("3. Médio (13-16 bits)");
    println!("4. Difícil (17-22 bits)");
    println!("5. Custom (defina bits)");
    
    // Obter escolha do usuário
    let difficulty = loop {
        print!("Escolha (1-5): ");
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                match input.trim().parse::<u8>() {
                    Ok(1..=5) => break input.trim().parse::<u8>().unwrap(),
                    _ => println!("{}Opção inválida, tente novamente.{}", colors::RED, colors::RESET)
                }
            },
            Err(_) => println!("{}Erro ao ler entrada, tente novamente.{}", colors::RED, colors::RESET)
        }
    };
    
    // Definir intervalo de bits com base na dificuldade
    let (min_bits, max_bits) = match difficulty {
        1 => (5, 8),
        2 => (9, 12),
        3 => (13, 16),
        4 => (17, 22),
        5 => {
            // Permitir personalização dos bits
            let bits = loop {
                print!("Digite o número de bits (5-22): ");
                io::stdout().flush().unwrap();
                
                let mut input = String::new();
                match io::stdin().read_line(&mut input) {
                    Ok(_) => {
                        match input.trim().parse::<u8>() {
                            Ok(b) if b >= 5 && b <= 22 => break b,
                            _ => println!("{}Valor inválido, digite um número entre 5 e 22.{}", colors::RED, colors::RESET)
                        }
                    },
                    Err(_) => println!("{}Erro ao ler entrada, tente novamente.{}", colors::RED, colors::RESET)
                }
            };
            (bits, bits)
        },
        _ => unreachable!()
    };
    
    // Selecionar um puzzle aleatório com a dificuldade escolhida
    let puzzles = bitcoin_puzzle_test::find_training_puzzles(min_bits, max_bits);
    
    if puzzles.is_empty() {
        println!("{}Não foram encontrados puzzles com essa dificuldade.{}", colors::RED, colors::RESET);
        return;
    }
    
    // Escolher um puzzle aleatório
    let mut rng = rand::thread_rng();
    let selected_puzzle = &puzzles[rng.gen_range(0..puzzles.len())];
    
    println!("\n{}Puzzle selecionado:{}", colors::BOLD_GREEN, colors::RESET);
    println!("{}Dificuldade: {} bits{}", colors::CYAN, selected_puzzle.bits, colors::RESET);
    println!("{}Endereço Bitcoin: {}{}", colors::CYAN, selected_puzzle.address, colors::RESET);
    println!("{}Hash160: {}{}", colors::CYAN, hex::encode(&selected_puzzle.hash160), colors::RESET);
    
    // No modo de treinamento, também mostramos a chave privada para fins educacionais
    println!("{}Chave privada (solução): {}{}", colors::MAGENTA, selected_puzzle.private_key, colors::RESET);
    
    // Criar intervalo de busca
    let key_value = u64::from_str_radix(&selected_puzzle.private_key, 16).unwrap_or(0);
    let range_start = key_value.saturating_sub(1000);
    let range_end = key_value.saturating_add(1000);
    
    println!("\n{}Intervalo de busca:{}", colors::YELLOW, colors::RESET);
    println!("{}De: {}{}", colors::CYAN, range_start, colors::RESET);
    println!("{}Até: {}{}", colors::CYAN, range_end, colors::RESET);
    
    // Calcular estimativa de tempo
    let range_size = range_end as u64 - range_start as u64;
    let estimated_time = range_size as f64 / est_keys_per_sec as f64;
    
    println!("\n{}Estimativa de tempo: {:.2} segundos{}", 
             colors::YELLOW, estimated_time, colors::RESET);
    
    // Criar chunks para processamento paralelo
    let min_key = BigUint::from(range_start);
    let max_key = BigUint::from(range_end);
    let chunks = performance::optimize_workload_distribution(&min_key, &max_key, params);
    
    println!("{}Iniciando busca com {} chunks...{}", 
             colors::GREEN, chunks.len(), colors::RESET);
    
    // Reset counter
    performance::reset_keys_checked();
    
    // Realizar a busca com parâmetros otimizados e possível aceleração por GPU
    let result = search::search_for_private_key_optimized(&chunks, &selected_puzzle.hash160, params.batch_size, gpu_searcher);
    
    // Processar o resultado
    if let Some(found_key_hex) = result {
        println!("\n{}CHAVE ENCONTRADA!{}", colors::BOLD_GREEN, colors::RESET);
        println!("{}Chave privada (hex): {}{}", colors::GREEN, found_key_hex, colors::RESET);
        
        // Verificar se a chave encontrada corresponde à esperada
        if found_key_hex == selected_puzzle.private_key {
            println!("{}SUCESSO! A chave encontrada é a correta!{}", colors::BOLD_GREEN, colors::RESET);
        } else {
            println!("{}ATENÇÃO: A chave encontrada é diferente da esperada!{}", colors::BOLD_YELLOW, colors::RESET);
            println!("{}Chave esperada: {}{}", colors::YELLOW, selected_puzzle.private_key, colors::RESET);
        }
        
        // Converter a chave hex para formato binário
        if let Ok(key_bytes) = hex::decode(&found_key_hex) {
            // Criar WIF e endereço
            match bitcoin::private_key_to_wif(&key_bytes) {
                Ok(wif) => println!("{}Chave privada (WIF): {}{}", colors::GREEN, wif, colors::RESET),
                Err(e) => println!("{}Erro ao gerar WIF: {:?}{}", colors::RED, e, colors::RESET),
            }
            
            match bitcoin::private_key_to_p2pkh_address(&key_bytes) {
                Ok(addr) => println!("{}Endereço Bitcoin: {}{}", colors::GREEN, addr, colors::RESET),
                Err(e) => println!("{}Erro ao gerar endereço: {:?}{}", colors::RED, e, colors::RESET),
            }
        } else {
            println!("{}Erro ao decodificar a chave hexadecimal{}", colors::RED, colors::RESET);
        }
    } else {
        println!("\n{}Busca concluída. Chave privada não encontrada neste intervalo.{}", 
               colors::RED, colors::RESET);
    }
}

fn run_range_test_mode(params: &performance::SearchParameters, est_keys_per_sec: u64) {
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
    print!("\n{}Escolha o número do puzzle para verificar (1-{}): {}", 
           colors::CYAN, puzzles.len(), colors::RESET);
    io::stdout().flush().unwrap();
    
    let stdin = io::stdin();
    let puzzle_choice_str = match stdin.lock().lines().next() {
        Some(Ok(line)) => line,
        _ => {
            println!("{}Falha ao ler entrada. Cancelando operação.{}", 
                     colors::YELLOW, colors::RESET);
            return;
        }
    };
    
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
    
    // Calcular tempo estimado usando a estimativa de performance específica para o hardware
    let est_seconds = keyspace_size.to_f64().unwrap() / est_keys_per_sec as f64;
    let est_days = est_seconds / (24.0 * 60.0 * 60.0);
    let est_years = est_days / 365.25;
    
    println!("\n{}Estimativa de tempo para busca completa (velocidade estimada: {:.2} M chaves/s):{}", 
             colors::YELLOW, est_keys_per_sec as f64 / 1_000_000.0, colors::RESET);
    
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
    
    // Visualização da distribuição de trabalho
    let chunks = performance::optimize_workload_distribution(&min_key, &max_key, params);
    println!("\n{}Estratégia de distribuição de trabalho otimizada:{}", colors::BOLD_YELLOW, colors::RESET);
    println!("{}Número de chunks para processamento paralelo: {}{}", colors::CYAN, chunks.len(), colors::RESET);
    
    println!("\n{}OBSERVAÇÃO: Este modo apenas verifica o intervalo sem iniciar a busca real.{}", 
             colors::BOLD_YELLOW, colors::RESET);
    println!("{}Para iniciar uma busca real neste puzzle, use o Modo Normal (opção 1).{}", 
             colors::YELLOW, colors::RESET);
}

fn run_normal_mode(params: &performance::SearchParameters, est_keys_per_sec: u64, gpu_searcher: OptionalGpuSearcher) {
    println!("\n{}Modo Normal - Bitcoin Puzzle TX{}", colors::BOLD_GREEN, colors::RESET);
    
    // Carregar puzzles
    let puzzles = bitcoin_puzzle::load_puzzles();
    if puzzles.is_empty() {
        println!("{}Não foram encontrados puzzles. Verifique os arquivos de dados.{}", 
                 colors::RED, colors::RESET);
        return;
    }
    
    // Mostrar lista de puzzles disponíveis
    println!("\n{}Puzzles disponíveis:{}", colors::BOLD_CYAN, colors::RESET);
    for (i, puzzle) in puzzles.iter().enumerate() {
        println!("{}{}. Dificuldade: {} bits | Endereço: {}{}", 
                 colors::CYAN, i+1, puzzle.bits, puzzle.address, colors::RESET);
    }
    
    // Permitir ao usuário escolher um puzzle
    let selected_idx = loop {
        print!("Escolha um puzzle para resolver (1-{}): ", puzzles.len());
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                match input.trim().parse::<usize>() {
                    Ok(num) if num >= 1 && num <= puzzles.len() => break num - 1,
                    _ => println!("{}Opção inválida, tente novamente.{}", colors::RED, colors::RESET)
                }
            },
            Err(_) => println!("{}Erro ao ler entrada, tente novamente.{}", colors::RED, colors::RESET)
        }
    };
    
    let selected_puzzle = &puzzles[selected_idx];
    
    println!("\n{}Puzzle selecionado:{}", colors::BOLD_GREEN, colors::RESET);
    println!("{}Dificuldade: {} bits{}", colors::CYAN, selected_puzzle.bits, colors::RESET);
    println!("{}Endereço Bitcoin: {}{}", colors::CYAN, selected_puzzle.address, colors::RESET);
    println!("{}Hash160: {}{}", colors::CYAN, hex::encode(&selected_puzzle.hash160), colors::RESET);
    
    // Definir o intervalo de busca com base na dificuldade
    let max_range = BigUint::from(1u8) << selected_puzzle.bits;
    let min_key = BigUint::from(0u8);
    
    println!("\n{}Intervalo de busca:{}", colors::YELLOW, colors::RESET);
    println!("{}De: 0{}", colors::CYAN, colors::RESET);
    println!("{}Até: {} (2^{}){}", colors::CYAN, max_range, selected_puzzle.bits, colors::RESET);
    
    // Calcular estimativa de tempo
    let range_size = max_range.to_f64().unwrap_or(f64::MAX);
    let estimated_time = range_size / est_keys_per_sec as f64;
    
    // Formatar tempo estimado
    let time_str = if estimated_time < 60.0 {
        format!("{:.2} segundos", estimated_time)
    } else if estimated_time < 3600.0 {
        format!("{:.2} minutos", estimated_time / 60.0)
    } else if estimated_time < 86400.0 {
        format!("{:.2} horas", estimated_time / 3600.0)
    } else if estimated_time < 31536000.0 {
        format!("{:.2} dias", estimated_time / 86400.0)
    } else {
        format!("{:.2} anos", estimated_time / 31536000.0)
    };
    
    println!("\n{}Estimativa de tempo: {}{}", colors::YELLOW, time_str, colors::RESET);
    
    // Confirmar se o usuário deseja continuar
    print!("Deseja continuar com a busca? (S/N): ");
    io::stdout().flush().unwrap();
    
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap_or(0);
    
    if !["s", "S", "sim", "Sim", "SIM"].contains(&input.trim()) {
        println!("{}Busca cancelada pelo usuário.{}", colors::YELLOW, colors::RESET);
        return;
    }
    
    // Criar chunks para processamento paralelo
    let chunks = performance::optimize_workload_distribution(&min_key, &max_range, params);
    
    println!("{}Iniciando busca com {} chunks...{}", 
             colors::GREEN, chunks.len(), colors::RESET);
    
    // Reset counter
    performance::reset_keys_checked();
    
    // Realizar a busca com parâmetros otimizados e possível aceleração por GPU
    let result = search::search_for_private_key_optimized(&chunks, &selected_puzzle.hash160, params.batch_size, gpu_searcher);
    
    // Processar o resultado
    if let Some(found_key_hex) = result {
        println!("\n{}CHAVE ENCONTRADA!{}", colors::BOLD_GREEN, colors::RESET);
        println!("{}Chave privada (hex): {}{}", colors::GREEN, found_key_hex, colors::RESET);
        
        // Converter a chave hex para formato binário
        if let Ok(key_bytes) = hex::decode(&found_key_hex) {
            // Criar WIF e endereço
            match bitcoin::private_key_to_wif(&key_bytes) {
                Ok(wif) => println!("{}Chave privada (WIF): {}{}", colors::GREEN, wif, colors::RESET),
                Err(e) => println!("{}Erro ao gerar WIF: {:?}{}", colors::RED, e, colors::RESET),
            }
            
            match bitcoin::private_key_to_p2pkh_address(&key_bytes) {
                Ok(addr) => println!("{}Endereço Bitcoin: {}{}", colors::GREEN, addr, colors::RESET),
                Err(e) => println!("{}Erro ao gerar endereço: {:?}{}", colors::RED, e, colors::RESET),
            }
            
            // Salvar resultados em arquivo
            let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
            let filename = format!("found_key_{}.txt", timestamp);
            
            if let Ok(mut file) = std::fs::File::create(&filename) {
                let _ = writeln!(file, "BITCOIN PUZZLE SOLUCIONADO!");
                let _ = writeln!(file, "Puzzle: {}", selected_puzzle.address);
                let _ = writeln!(file, "Dificuldade: {} bits", selected_puzzle.bits);
                let _ = writeln!(file, "Chave privada (hex): {}", found_key_hex);
                
                if let Ok(wif) = bitcoin::private_key_to_wif(&key_bytes) {
                    let _ = writeln!(file, "Chave privada (WIF): {}", wif);
                }
                
                if let Ok(addr) = bitcoin::private_key_to_p2pkh_address(&key_bytes) {
                    let _ = writeln!(file, "Endereço Bitcoin: {}", addr);
                }
                
                println!("{}Resultados salvos em '{}'{}", colors::YELLOW, filename, colors::RESET);
            }
        } else {
            println!("{}Erro ao decodificar a chave hexadecimal{}", colors::RED, colors::RESET);
        }
    } else {
        println!("\n{}Busca concluída. Chave privada não encontrada neste intervalo.{}", 
               colors::RED, colors::RESET);
    }
} 