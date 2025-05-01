# Bitcoin Private Key Finder (Rust)

Esta é uma implementação em Rust de alta performance para busca de chaves privadas Bitcoin dentro de intervalos específicos, com suporte especial para os desafios Bitcoin Puzzle TX.

## Características

1. **Modelo de concorrência eficiente** usando threads leves do Rust e operações atômicas
2. **Uso eficiente de memória** com melhor gerenciamento e operações zero-copy
3. **Sincronização eficiente de threads** usando `Arc` e `Mutex` para estado compartilhado
4. **Operações criptográficas otimizadas** via bibliotecas Bitcoin para Rust
5. **Otimizações de compilador** habilitadas com LTO (Link Time Optimization) e otimizações agressivas
6. **Detecção automática de recursos** com ajuste dinâmico de performance

## Requisitos

- Rust 1.50 ou superior

## Compilação

```bash
# Build de desenvolvimento
cargo build

# Build de produção (muito mais rápido)
cargo build --release
```

## Execução

```bash
# Executar o build de produção
cargo run --release
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

O programa oferece três modos de operação distintos:

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

## Licença

MIT
