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
use num_bigint::BigUint;
use num_traits::ToPrimitive;
use hex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

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
    let gpu_searcher: OptionalGpuSearcher = if use_gpu {
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
    let base_keys_per_sec = performance::estimate_search_speed(&resources, &params);
    
    // Ajuste da estimativa de velocidade se usando GPU
    let (est_keys_per_sec, gpu_searcher) = if gpu_searcher.is_some() {
        // As GPUs geralmente são 10-100x mais rápidas para este tipo de operação
        let gpu_adjusted_speed = base_keys_per_sec * 20; // Multiplicador conservador
        println!("{}Velocidade estimada com GPU: {:.2} M chaves/s{}\n", 
                colors::GREEN, gpu_adjusted_speed as f64 / 1_000_000.0, colors::RESET);
        (gpu_adjusted_speed, gpu_searcher)
    } else {
        println!("{}Velocidade estimada: {:.2} M chaves/s{}\n", 
                colors::GREEN, base_keys_per_sec as f64 / 1_000_000.0, colors::RESET);
        (base_keys_per_sec, None)
    };
    
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
    };
}

fn run_training_mode(params: &performance::SearchParameters, est_keys_per_sec: u64, gpu_searcher: OptionalGpuSearcher) -> Option<String> {
    println!("{}Modo Treinamento - Bitcoin Puzzles conhecidos{}", colors::BOLD_GREEN, colors::RESET);
    println!("{}Neste modo, vamos buscar chaves privadas já conhecidas para validar o funcionamento do sistema.{}", 
             colors::YELLOW, colors::RESET);
    
    // Carregar puzzles de treinamento
    let puzzles = match find_training_puzzles() {
        Some(p) => p,
        None => {
            println!("{}Erro ao carregar puzzles de treinamento.{}", colors::RED, colors::RESET);
            return None;
        }
    };
    
    // Mostrar puzzles disponíveis
    println!("\n{}Puzzles disponíveis para treinamento:{}", colors::BOLD_CYAN, colors::RESET);
    for (i, puzzle) in puzzles.iter().enumerate() {
        println!("{}. Dificuldade: {} bits | Endereço: {}", 
                 i+1, puzzle.bits, puzzle.address);
    }
    
    // Selecionar puzzle
    println!("\n{}Digite o número do puzzle que deseja testar, ou 0 para testar todos:{}", 
             colors::BOLD_YELLOW, colors::RESET);
    let mut selection = String::new();
    std::io::stdin().read_line(&mut selection).expect("Falha ao ler entrada");
    let index = selection.trim().parse::<usize>().unwrap_or(0);
    
    if index == 0 {
        // Executar todos os puzzles em ordem de dificuldade
        run_all_training_puzzles(&puzzles, params, est_keys_per_sec, gpu_searcher);
        None
    } else if index <= puzzles.len() {
        // Executar apenas o puzzle selecionado
        let puzzle = &puzzles[index-1];
        if run_single_training_puzzle(puzzle, params, est_keys_per_sec, gpu_searcher) {
            Some(puzzle.private_key.clone())
        } else {
            None
        }
    } else {
        println!("{}Seleção inválida.{}", colors::RED, colors::RESET);
        None
    }
}

fn run_all_training_puzzles(
    puzzles: &[bitcoin_puzzle_test::TestPuzzle], 
    params: &performance::SearchParameters, 
    est_keys_per_sec: u64,
    gpu_searcher: OptionalGpuSearcher
) {
    let mut success_count = 0;
    let start_time = std::time::Instant::now();
    
    println!("{}Iniciando teste de todos os puzzles...{}", colors::BOLD_GREEN, colors::RESET);
    
    // Ordenar puzzles por dificuldade (bits)
    let mut sorted_puzzles = puzzles.to_vec();
    sorted_puzzles.sort_by_key(|p| p.bits);
    
    for (i, puzzle) in sorted_puzzles.iter().enumerate() {
        println!("\n{}Testando puzzle {}/{} ({} bits){}", 
                 colors::BOLD_CYAN, i+1, sorted_puzzles.len(), puzzle.bits, colors::RESET);
        
        // Create a new GPU searcher for each puzzle (to avoid ownership issues)
        #[cfg(feature = "opencl")]
        let puzzle_gpu_searcher = match &gpu_searcher {
            Some(_) => {
                // Create a new GPU searcher
                match crate::gpu::GpuSearcher::new() {
                    Ok(mut new_searcher) => {
                        // Initialize with the same device
                        if let Err(e) = new_searcher.select_device(0) {
                            println!("{}Erro ao selecionar dispositivo GPU: {}{}", 
                                     colors::YELLOW, e, colors::RESET);
                            None
                        } else if let Err(e) = new_searcher.initialize_program() {
                            println!("{}Erro ao inicializar programa OpenCL: {}{}", 
                                     colors::YELLOW, e, colors::RESET);
                            None
                        } else {
                            Some(new_searcher)
                        }
                    },
                    Err(e) => {
                        println!("{}Erro ao criar novo GPU searcher: {}{}", 
                                 colors::YELLOW, e, colors::RESET);
                        None
                    }
                }
            },
            None => None
        };
        
        #[cfg(not(feature = "opencl"))]
        let puzzle_gpu_searcher = None;
        
        // Executar o puzzle
        let found = run_single_training_puzzle(puzzle, params, est_keys_per_sec, puzzle_gpu_searcher);
        
        if found {
            success_count += 1;
        }
    }
    
    // Exibir resultados
    let elapsed = start_time.elapsed().as_secs();
    println!("\n{}Resultados do treinamento:{}", colors::BOLD_GREEN, colors::RESET);
    println!("{}Total de puzzles: {}{}", colors::CYAN, sorted_puzzles.len(), colors::RESET);
    println!("{}Puzzles resolvidos: {}{}", colors::CYAN, success_count, colors::RESET);
    println!("{}Tempo total: {} segundos{}", colors::CYAN, elapsed, colors::RESET);
}

fn run_single_training_puzzle(
    puzzle: &bitcoin_puzzle_test::TestPuzzle, 
    params: &performance::SearchParameters, 
    est_keys_per_sec: u64,
    gpu_searcher: OptionalGpuSearcher
) -> bool {
    // Mostrar informações do puzzle
    bitcoin_puzzle_test::display_test_puzzle_info(puzzle);
    
    // Calcular range
    let range = bitcoin_puzzle_test::puzzle_to_range(puzzle);
    
    // Mostrar informações da busca
    // Convert the string range to BigUint for calculations
    let min = num_bigint::BigUint::parse_bytes(range.min.trim_start_matches("0x").as_bytes(), 16).unwrap();
    let max = num_bigint::BigUint::parse_bytes(range.max.trim_start_matches("0x").as_bytes(), 16).unwrap();
    let range_size = &max - &min;
    
    let keys_per_sec = est_keys_per_sec; // Always use the estimated keys per second
    let est_seconds = range_size.to_f64().unwrap_or(f64::MAX) / keys_per_sec as f64;
    
    println!("{}Intervalo de busca:{}", colors::BOLD_CYAN, colors::RESET);
    println!("{}De: {}{}", colors::CYAN, range.min, colors::RESET);
    
    // Check bit size using the BigUint instead of String
    if max.bits() <= 64 {
        println!("{}Até: {} (2^{}){}", colors::CYAN, range.max, puzzle.bits, colors::RESET);
    } else {
        println!("{}Até: 2^{}{}", colors::CYAN, puzzle.bits, colors::RESET);
    }
    
    if est_seconds < 0.001 {
        println!("{}Estimativa de tempo: <1ms{}", colors::YELLOW, colors::RESET);
    } else if est_seconds < 1.0 {
        println!("{}Estimativa de tempo: {:.1} ms{}", colors::YELLOW, est_seconds * 1000.0, colors::RESET);
    } else if est_seconds < 60.0 {
        println!("{}Estimativa de tempo: {:.2} segundos{}", colors::YELLOW, est_seconds, colors::RESET);
    } else {
        println!("{}Estimativa de tempo: {:.2} minutos{}", colors::YELLOW, est_seconds / 60.0, colors::RESET);
    }
    
    // Criar ranges para busca - adjust to use params.threads instead of num_threads
    let chunks = std::cmp::min(44, params.threads * 4);
    let mut ranges = Vec::with_capacity(chunks as usize);
    
    // Dividir o intervalo em partes iguais
    let chunk_size = if range_size > (chunks as u32).into() {
        &range_size / chunks
    } else {
        range_size.clone()
    };
    
    let mut start = min.clone();
    for i in 0..chunks {
        let end = if i == chunks - 1 {
            max.clone()
        } else {
            &start + &chunk_size
        };
        
        ranges.push((start.clone(), end.clone()));
        start = end + num_bigint::BigUint::from(1u32);  // Explicitly convert 1u32 to BigUint
    }
    
    // Iniciar a busca
    println!("{}Iniciando busca...{}", colors::BOLD_GREEN, colors::RESET);
    
    // Chamar a função de busca
    let found_key = search::search_for_private_key_optimized(&ranges, &puzzle.hash160, params.batch_size, gpu_searcher);
    
    if let Some(key) = found_key {
        // Verificar se a chave encontrada é a correta
        let expected_key = &puzzle.private_key;
        
        // Normalizar chaves para comparação (remover zeros à esquerda)
        let found_key_normalized = key.trim_start_matches('0');
        let expected_normalized = expected_key.trim_start_matches('0');
        
        if found_key_normalized.eq_ignore_ascii_case(expected_normalized) {
            println!("\n{}SUCESSO! Chave encontrada corretamente!{}", colors::BOLD_GREEN, colors::RESET);
            println!("{}Chave esperada: {}{}", colors::GREEN, expected_key, colors::RESET);
            println!("{}Chave encontrada: {}{}", colors::GREEN, key, colors::RESET);
            true
        } else {
            println!("\n{}AVISO: Chave encontrada não corresponde à esperada!{}", colors::RED, colors::RESET);
            println!("{}Chave esperada: {}{}", colors::YELLOW, expected_key, colors::RESET);
            println!("{}Chave encontrada: {}{}", colors::RED, key, colors::RESET);
            false
        }
    } else {
        println!("\n{}FALHA: Chave não encontrada.{}", colors::RED, colors::RESET);
        println!("{}Chave esperada: {}{}", colors::YELLOW, puzzle.private_key, colors::RESET);
        false
    }
}

// Helper function to clone the GPU searcher if available
#[cfg(feature = "opencl")]
trait GpuSearcherExt {
    fn clone_if_available(self) -> Self;
}

#[cfg(feature = "opencl")]
impl GpuSearcherExt for OptionalGpuSearcher {
    fn clone_if_available(self) -> Self {
        match self {
            Some(_) => {
                // Create a new GPU searcher
                match crate::gpu::GpuSearcher::new() {
                    Ok(mut new_searcher) => {
                        // Initialize with the same device
                        if let Err(e) = new_searcher.select_device(0) {
                            println!("{}Erro ao selecionar dispositivo GPU: {}{}", 
                                     colors::YELLOW, e, colors::RESET);
                            return None;
                        }
                        
                        // Initialize OpenCL program
                        if let Err(e) = new_searcher.initialize_program() {
                            println!("{}Erro ao inicializar programa OpenCL: {}{}", 
                                     colors::YELLOW, e, colors::RESET);
                            return None;
                        }
                        
                        Some(new_searcher)
                    },
                    Err(e) => {
                        println!("{}Erro ao criar novo GPU searcher: {}{}", 
                                 colors::YELLOW, e, colors::RESET);
                        None
                    }
                }
            },
            None => None
        }
    }
}

#[cfg(not(feature = "opencl"))]
trait GpuSearcherExt {
    fn clone_if_available(self) -> Self;
}

#[cfg(not(feature = "opencl"))]
impl GpuSearcherExt for OptionalGpuSearcher {
    fn clone_if_available(self) -> Self {
        None
    }
}

// This function now needs to be implemented
fn find_training_puzzles() -> Option<Vec<bitcoin_puzzle_test::TestPuzzle>> {
    // Get test puzzles with bits between 5 and 22
    let mut puzzles = bitcoin_puzzle_test::find_training_puzzles(5, 22);
    
    // Convert addresses to hash160
    if let Err(e) = bitcoin_puzzle_test::convert_addresses_to_hash160(&mut puzzles) {
        println!("{}Erro ao converter endereços para hash160: {}{}", 
                 colors::RED, e, colors::RESET);
        return None;
    }
    
    Some(puzzles)
}

fn run_range_test_mode(params: &performance::SearchParameters, est_keys_per_sec: u64) -> Option<String> {
    println!("{}Modo de TESTE DE RANGES ativado - Verificando intervalos dos puzzles não resolvidos{}", colors::BOLD_GREEN, colors::RESET);
    
    // Carregar os puzzles Bitcoin não resolvidos
    let mut puzzles = bitcoin_puzzle::get_unsolved_puzzles();
    match bitcoin_puzzle::convert_addresses_to_hash160(&mut puzzles) {
        Ok(_) => (),
        Err(err) => {
            println!("{}Erro ao converter endereços para hash160: {}{}", colors::RED, err, colors::RESET);
            return None;
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
            return None;
        }
    };
    
    let puzzle_choice: usize = match puzzle_choice_str.trim().parse() {
        Ok(num) if num >= 1 && num <= puzzles.len() => num,
        _ => {
            println!("{}Escolha inválida. Por favor, escolha um número entre 1 e {}.{}", 
                     colors::RED, puzzles.len(), colors::RESET);
            return None;
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
    let num_chunks = params.threads; // Usar o número de threads como número de chunks
    let chunks = search::create_search_chunks(min_key.clone(), max_key.clone(), num_chunks);
    println!("\n{}Estratégia de distribuição de trabalho otimizada:{}", colors::BOLD_YELLOW, colors::RESET);
    println!("{}Número de chunks para processamento paralelo: {}{}", colors::CYAN, chunks.len(), colors::RESET);
    
    println!("\n{}OBSERVAÇÃO: Este modo apenas verifica o intervalo sem iniciar a busca real.{}", 
             colors::BOLD_YELLOW, colors::RESET);
    println!("{}Para iniciar uma busca real neste puzzle, use o Modo Normal (opção 1).{}", 
             colors::YELLOW, colors::RESET);
             
    None
}

fn run_normal_mode(params: &performance::SearchParameters, est_keys_per_sec: u64, gpu_searcher: OptionalGpuSearcher) -> Option<String> {
    println!("\n{}Modo Normal - Bitcoin Puzzle TX{}", colors::BOLD_GREEN, colors::RESET);
    
    // Carregar puzzles
    let puzzles = match load_bitcoin_puzzles() {
        Ok(p) => p,
        Err(e) => {
            println!("{}Erro ao carregar puzzles: {}{}", colors::RED, e, colors::RESET);
            return None;
        }
    };
    
    if puzzles.is_empty() {
        println!("{}Não foram encontrados puzzles. Verifique os arquivos de dados.{}", 
                 colors::RED, colors::RESET);
        return None;
    }
    
    // Exibir puzzles disponíveis
    print_puzzles(&puzzles);
    
    // Selecionar puzzle
    let puzzle_index = select_puzzle_index(&puzzles);
    let selected_puzzle = &puzzles[puzzle_index];
    
    println!("\n{}Puzzle selecionado:{}", colors::BOLD_GREEN, colors::RESET);
    println!("{}Dificuldade: {} bits", colors::GREEN, selected_puzzle.bits);
    println!("Endereço Bitcoin: {}", selected_puzzle.address);
    println!("Hash160: {}{}", hex::encode(&selected_puzzle.hash160), colors::RESET);
    
    // Calcular intervalo de busca
    let (min_key, max_key) = get_search_range(selected_puzzle.bits);
    
    println!("\n{}Intervalo de busca:{}", colors::BOLD_CYAN, colors::RESET);
    println!("{}De: {}", colors::CYAN, min_key);
    println!("Até: {} (2^{}){}", max_key, selected_puzzle.bits, colors::RESET);
    
    // Estimar tempo
    let key_range = max_key.clone() - min_key.clone();
    let key_range_f64 = key_range.to_f64().unwrap_or(f64::MAX);
    let est_secs = key_range_f64 / est_keys_per_sec as f64;
    
    let est_years = est_secs / (60.0 * 60.0 * 24.0 * 365.25);
    println!("\n{}Estimativa de tempo: {:.2} anos{}", 
             if est_years > 100.0 { colors::RED } else { colors::YELLOW }, 
             est_years, colors::RESET);
    
    // Confirmar busca
    println!("{}Deseja continuar com a busca? (S/N): {}", colors::BOLD_WHITE, colors::RESET);
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).expect("Falha ao ler input");
    
    if !["s", "S", "sim", "Sim", "SIM"].contains(&input.trim()) {
        println!("{}Busca cancelada pelo usuário.{}", colors::YELLOW, colors::RESET);
        return None;
    }
    
    // Dividir em chunks
    let chunks = search::create_search_chunks(min_key, max_key, params.threads);
    println!("Iniciando busca com {} chunks...", chunks.len());
    
    // Resetar contadores
    performance::reset_keys_checked();
    
    // Thread de progresso para mostrar informações enquanto a busca é executada
    let show_progress = Arc::new(AtomicBool::new(true));
    let show_progress_clone = show_progress.clone();
    let range_bits = selected_puzzle.bits;

    let progress_thread = std::thread::spawn(move || {
        let start_time = std::time::Instant::now();
        let mut last_keys_checked = 0;
        let mut last_check_time = start_time;
        
        while show_progress_clone.load(Ordering::Relaxed) {
            std::thread::sleep(std::time::Duration::from_secs(1));
            
            let current_time = std::time::Instant::now();
            let total_keys_checked = performance::get_keys_checked();
            let time_diff = current_time.duration_since(last_check_time).as_secs_f64();
            
            if time_diff > 0.0 {
                let keys_per_second = (total_keys_checked - last_keys_checked) as f64 / time_diff;
                let total_time = current_time.duration_since(start_time).as_secs_f64();
                let overall_speed = if total_time > 0.0 { total_keys_checked as f64 / total_time } else { 0.0 };
                
                print!("\r\x1b[K");  // Limpa a linha atual
                print!("{}[{}] {:.2}M chaves/s | {:.2}M média | Total: {} chaves ({:.6}%){}", 
                       colors::CYAN,
                       chrono::Local::now().format("%H:%M:%S"),
                       keys_per_second / 1_000_000.0,
                       overall_speed / 1_000_000.0,
                       total_keys_checked,
                       if range_bits > 0 { total_keys_checked as f64 / (2.0_f64.powf(range_bits as f64)) * 100.0 } else { 0.0 },
                       colors::RESET);
                std::io::stdout().flush().unwrap();
                
                last_keys_checked = total_keys_checked;
                last_check_time = current_time;
            }
        }
    });

    // Realizar a busca com parâmetros otimizados e possível aceleração por GPU
    let result = search::search_for_private_key_optimized(&chunks, &selected_puzzle.hash160, params.batch_size, gpu_searcher);
    
    // Parar o thread de progresso
    show_progress.store(false, Ordering::Relaxed);
    let _ = progress_thread.join();
    
    // Processar o resultado
    if let Some(found_key_hex) = &result {
        println!("\n\n{}CHAVE ENCONTRADA!{}", colors::BOLD_GREEN, colors::RESET);
        println!("{}Chave privada (hex): {}{}", colors::GREEN, found_key_hex, colors::RESET);
        
        // BitcoinPuzzle não tem o campo private_key para puzzles reais
        println!("{}VERIFICAÇÃO: A chave privada foi encontrada mas não pode ser validada (puzzle real).{}", 
                colors::BOLD_YELLOW, colors::RESET);
        
        // Converter a chave hex para formato binário
        if let Ok(key_bytes) = hex::decode(found_key_hex) {
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
        println!("\n\n{}Busca concluída. Chave privada não encontrada neste intervalo.{}", 
               colors::RED, colors::RESET);
    }
    
    result
}

// Carregar os puzzles do Bitcoin
fn load_bitcoin_puzzles() -> Result<Vec<bitcoin_puzzle::BitcoinPuzzle>, String> {
    match bitcoin_puzzle::load_puzzles() {
        puzzles if !puzzles.is_empty() => Ok(puzzles),
        _ => Err("Nenhum puzzle encontrado".to_string())
    }
}

// Exibir puzzles disponíveis
fn print_puzzles(puzzles: &[bitcoin_puzzle::BitcoinPuzzle]) {
    println!("\n{}Puzzles disponíveis:{}", colors::BOLD_CYAN, colors::RESET);
    for (i, puzzle) in puzzles.iter().enumerate() {
        println!("{}{}. Dificuldade: {} bits | Endereço: {}{}", 
                colors::CYAN, i+1, puzzle.bits, puzzle.address, colors::RESET);
    }
}

// Permitir ao usuário selecionar um puzzle
fn select_puzzle_index(puzzles: &[bitcoin_puzzle::BitcoinPuzzle]) -> usize {
    loop {
        print!("Escolha um puzzle para resolver (1-{}): ", puzzles.len());
        std::io::stdout().flush().unwrap();
        
        let mut input = String::new();
        match std::io::stdin().read_line(&mut input) {
            Ok(_) => {
                match input.trim().parse::<usize>() {
                    Ok(num) if num >= 1 && num <= puzzles.len() => return num - 1,
                    _ => println!("{}Opção inválida, tente novamente.{}", colors::RED, colors::RESET)
                }
            },
            Err(_) => println!("{}Erro ao ler entrada, tente novamente.{}", colors::RED, colors::RESET)
        }
    }
}

// Calcular intervalo de busca com base na dificuldade do puzzle
fn get_search_range(bits: u32) -> (BigUint, BigUint) {
    let max_range = BigUint::from(1u8) << bits;
    let min_key = BigUint::from(0u8);
    (min_key, max_range)
} 