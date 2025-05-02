# Bitcoin Private Key Finder (Rust)

A high-performance Bitcoin private key finder tool written in Rust, with both CPU and GPU support.

## Features

- Searches for Bitcoin private keys based on given criteria
- High-performance multi-threaded CPU search
- GPU acceleration using OpenCL (now using opencl3 library)
- Dynamic hardware detection and optimization
- Automatic resource usage tuning
- Support for Bitcoin puzzles and specific key ranges
- MiMalloc memory allocator for improved performance

## Requirements

- Rust 1.65 or newer
- For GPU acceleration:
  - OpenCL 1.2+ capable hardware and drivers
  - OpenCL development headers/libraries

## Building

### Basic CPU-only build (Recommended):

```bash
# Build in release mode for best performance
cargo build --release
```

### With GPU (OpenCL) support:

```bash
# Build with OpenCL support
cargo build --release --features opencl
```

### With MiMalloc memory allocator:

```bash
# Build with optimized memory allocator
cargo build --release --features mimalloc
```

### All features:

```bash
# Build with all optimizations
cargo build --release --features "opencl mimalloc"
```

## Running

```bash
# Run the program
./target/release/btcrustai
```

## Available Modes

1. **Normal Mode** - Attempts to solve real Bitcoin Puzzle TX challenges
2. **Training Mode** - Uses small puzzles (5-22 bits) with known keys
3. **Range Test Mode** - Verifies ranges of unsolved puzzles without actual search

## Performance

The application automatically detects your hardware capabilities and adjusts for optimal performance. You can control resource usage through command-line options:

```bash
# Use 50% of system resources
./target/release/btcrustai --usage 50
```

## Bitcoin Puzzle TX Challenge

This tool supports the Bitcoin Puzzle TX challenge (from privatekeys.pw/puzzles/bitcoin-puzzle-tx), focusing on finding private keys for specific unsolved puzzles (numbers 71-160, excluding solved ones).

## OpenCL Support

OpenCL support has been upgraded to use the opencl3 library for better compatibility. For detailed information about OpenCL support, see [OPENCL_SUPPORT.md](OPENCL_SUPPORT.md).

## License

This project is open source and available under the MIT License.

## Características

1. **Modelo de concorrência eficiente** usando threads leves do Rust e operações atômicas
2. **Uso eficiente de memória** com melhor gerenciamento e operações zero-copy
3. **Sincronização eficiente de threads** usando `Arc` e `Mutex` para estado compartilhado
4. **Operações criptográficas otimizadas** via bibliotecas Bitcoin para Rust
5. **Otimizações de compilador** habilitadas com LTO (Link Time Optimization) e otimizações agressivas
6. **Detecção automática de recursos** com ajuste dinâmico de performance

## Requisitos

- Rust 1.50 ou superior
- Dependências incluídas no Cargo.toml:
  - bitcoin, bitcoin_hashes, secp256k1 (operações criptográficas)
  - rayon (paralelismo)
  - mimalloc (alocador de memória otimizado)
  - bs58, hex (codificação/decodificação)
  - serde, serde_json (serialização)
  - num-bigint, num-traits (aritmética com números grandes)
  - sysinfo (informações do sistema)

## Compilação

```bash
# Build de desenvolvimento
cargo build

# Build de produção (muito mais rápido)
cargo build --release

# Build com suporte a OpenCL (GPU) 
cargo build --release --features opencl
```

O projeto agora detecta automaticamente a presença e disponibilidade do OpenCL, 
com fallback transparente para CPU quando a GPU não está disponível.

## Execução

```bash
# Executar o build de produção
cargo run --release

# Passando argumentos específicos
cargo run --release -- --mode training
```

## Detecção de Recursos e Otimização

O programa analisa automaticamente os recursos do sistema para otimizar a performance:

1. **Detecção de hardware**:
   - Número de cores físicos e threads lógicos da CPU
   - Memória total e disponível
   - Conjunto de instruções SIMD disponíveis (AVX2, AVX, SSE)
   - Marca e modelo da CPU

2. **Ajuste dinâmico**:
   - Controle de uso de recursos (percentual configurável)
   - Tamanho de batch otimizado com base na memória disponível
   - Distribuição eficiente de trabalho entre threads
   - Estimativas de performance personalizadas para o hardware específico

3. **Monitoramento em tempo real**:
   - Contagem global de chaves verificadas
   - Velocidade instantânea e média
   - Estatísticas de progresso

O usuário pode escolher a porcentagem de recursos do sistema que deseja utilizar (10-100%), permitindo executar outras tarefas enquanto a busca ocorre em segundo plano.

## Modos de Operação

O programa oferece quatro modos de operação distintos:

### 1. Modo Normal

Este é o modo principal para tentar encontrar chaves privadas dos puzzles reais do [Bitcoin Puzzle TX Challenge](https://privatekeys.pw/puzzles/bitcoin-puzzle-tx). Neste modo:
1. São apresentados todos os puzzles não resolvidos (71-160)
2. Você seleciona um puzzle específico
3. O programa inicia a busca efetiva pela chave privada usando todos os cores disponíveis

### 2. Modo Treinamento

Utiliza puzzles de baixíssima dificuldade (5, 15 e 22 bits) que podem ser resolvidos em segundos ou minutos para verificar o funcionamento correto do algoritmo de busca. Estes puzzles têm chaves privadas conhecidas, ideais para:
- Testar a instalação do programa
- Verificar se o algoritmo de busca está funcionando corretamente
- Demonstrar o processo de descoberta de chaves

```bash
# Executar no modo treinamento
cargo run --release -- --mode training
```

### 3. Modo Teste de Ranges

Este modo permite explorar os intervalos de busca dos puzzles reais não resolvidos, mas sem iniciar a busca efetiva. É útil para:
- Analisar a dificuldade de cada puzzle
- Ver estimativas de tempo para busca completa
- Examinar os intervalos de chaves privadas
- Obter estatísticas sobre probabilidades de sucesso

```bash
# Executar no modo teste de ranges
cargo run --release -- --mode range
```

Este modo é recomendado para entender a magnitude do desafio antes de tentar a busca real no Modo Normal.

### 4. Modo Bitcoin Puzzle

Este modo foca especificamente nos desafios do Bitcoin Puzzle TX, tentando resolver os puzzles não resolvidos do privatekeys.pw.

```bash
# Executar no modo puzzle
cargo run --release -- --mode puzzle
```

## Otimizações de Performance

Esta implementação inclui diversas otimizações de performance:

- Uso de Rayon para processamento paralelo
- Alocador de memória Mimalloc para melhor desempenho de memória
- Fat LTO (Link Time Optimization)
- Processamento de chaves em batch para melhor localidade de cache
- Funções de comparação de bytes otimizadas
- Operações zero-copy quando possível
- Otimizações de compilador com configurações agressivas
- Perfil de release otimizado no Cargo.toml

## Estrutura do Projeto

O projeto é organizado em módulos específicos:

- **main.rs**: Ponto de entrada, interface de usuário e seleção de modos
- **bitcoin.rs**: Implementações específicas para Bitcoin (derivação de chaves, etc.)
- **bitcoin_puzzle.rs**: Implementação dos desafios Bitcoin Puzzle TX
- **bitcoin_puzzle_test.rs**: Configurações para o modo de treinamento
- **search.rs**: Algoritmos de busca e verificação de chaves
- **performance.rs**: Detecção de recursos e otimizações
- **models.rs**: Estruturas de dados compartilhadas
- **colors.rs**: Formatação de cores para terminal
- **data.rs**: Dados estáticos e constantes

## Desenvolvimento

### Arquivos Ignorados

Este projeto utiliza `.gitignore` para excluir do controle de versão:

- Diretórios `/target/` com binários compilados
- Arquivos de debug (*.pdb)
- Executáveis (*.exe, *.dll, *.so, *.dylib)
- Arquivos temporários de sistema (.DS_Store, Thumbs.db)
- Arquivos de configuração de IDE (.idea/, .vscode/)
- Arquivos de log e variáveis de ambiente

Isso mantém o repositório limpo, contendo apenas código fonte e documentação essencial.

## Funcionalidades

- Busca de chaves privadas para puzzles Bitcoin não resolvidos
- Interface de usuário interativa para seleção de puzzles
- Estatísticas de busca em tempo real
- Suporte multi-thread para máxima performance
- Salvamento automático de resultados encontrados
- Modo de treinamento com puzzles de baixa dificuldade para verificação do funcionamento
- Modo de teste de ranges para análise da magnitude do desafio
- Ajuste automático de parâmetros com base no hardware disponível
- Suporte para endereços P2PKH (começando com '1')
- Conversão automática de endereços para hash160
- Base de dados interna com puzzles não resolvidos

## Licença

MIT

## GPU Acceleration with OpenCL

O projeto agora inclui suporte opcional para aceleração via GPU usando OpenCL. Esta funcionalidade permite processar muito mais chaves por segundo em hardware compatível.

### Requisitos para GPU

- Drivers gráficos atualizados com suporte a OpenCL
- Uma das seguintes GPUs:
  - NVIDIA GeForce (drivers CUDA)
  - AMD Radeon (drivers AMD APP/ROCm)
  - Intel Graphics (drivers OpenCL para Intel)

### Compilação com suporte a GPU

```bash
# Compilar com suporte a OpenCL (Linux/macOS/Windows)
cargo build --release --features opencl
```

### Troubleshooting OpenCL

Se você estiver tendo problemas com o OpenCL, consulte o arquivo OPENCL_SUPPORT.md para instruções detalhadas.

Alternativamente, você pode simplesmente compilar sem suporte a GPU:

```bash
# Compilar sem suporte a GPU
cargo build --release
```

Se você quiser o suporte completo a GPU, certifique-se de que:
1. Seus drivers de GPU estão atualizados
2. Você tem o OpenCL instalado com seus drivers
3. Em algumas placas NVIDIA, pode ser necessário instalar o CUDA Toolkit

### Novo Sistema de Carregamento Dinâmico

O projeto agora utiliza carregamento dinâmico para OpenCL, o que significa:
- Não é necessário ter bibliotecas estáticas (.lib, .a) em tempo de compilação
- O programa detecta e carrega o OpenCL em tempo de execução
- Melhor compatibilidade entre diferentes sistemas operacionais
- Fallback automático para CPU se o OpenCL não estiver disponível

### Uso da GPU

Quando executado com suporte a GPU, o programa fará automaticamente:

1. Detecção de dispositivos OpenCL disponíveis
2. Listagem de todas as GPUs compatíveis encontradas
3. Pergunta se você deseja usar aceleração por GPU
4. Em caso afirmativo, permite selecionar o dispositivo específico
5. Usa a GPU para processamento, com fallback para CPU em caso de erros

A aceleração por GPU pode oferecer ganhos de 10-100x na velocidade de busca, dependendo do hardware.

### Otimizações para Ranges Grandes

O projeto agora divide automaticamente os espaços de busca muito grandes em chunks menores, permitindo:
- Processar intervalos de busca que anteriormente seriam muito grandes para GPU
- Utilizar a GPU de forma mais eficiente
- Evitar problemas de estouro de buffer na GPU

### Configurações Avançadas para GPU

O código inclui vários parâmetros configuráveis para otimizar o desempenho da GPU:

- Tamanho dos workgroups OpenCL ajustável
- Processamento em chunks para intervalos grandes
- Distribuição inteligente de trabalho entre CPU e GPU
- Fallback automático para CPU em caso de falha
- Kernels OpenCL otimizados para cálculos com Hash160 (SHA256+RIPEMD160)
