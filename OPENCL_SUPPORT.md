# OpenCL Support for Bitcoin Private Key Finder

Este documento explica como utilizar o suporte a GPU via OpenCL neste projeto.

## Requisitos

- GPU com suporte a OpenCL (NVIDIA, AMD, Intel)
- Drivers gráficos atualizados com OpenCL instalado
- Nenhuma biblioteca estática adicional é necessária (carregamento dinâmico)

## Status Atual do Suporte a OpenCL

**NOTA IMPORTANTE**: O suporte a OpenCL foi atualizado para usar a biblioteca `opencl3` com carregamento dinâmico. Esta atualização elimina a necessidade de vincular bibliotecas estáticas do OpenCL durante a compilação, tornando o processo muito mais simples e robusto.

## Métodos de Compilação

### Método 1: Compilação Segura (Recomendado para iniciantes)

```bash
# Compilação padrão sem suporte a GPU
cargo build --release
```

### Método 2: Compilação Com OpenCL (Recomendado)

```bash
# Compilar com suporte a OpenCL (carregamento dinâmico)
cargo build --release --features opencl
```

### Método 3: Com Alocador Otimizado (MiMalloc)

```bash
# Compilar com alocador otimizado
cargo build --release --features mimalloc

# Ou combinando com OpenCL
cargo build --release --features "opencl mimalloc"
```

## Funcionalidades do Suporte a OpenCL

- Detecção automática de GPUs compatíveis com OpenCL
- Carregamento dinâmico da biblioteca OpenCL em tempo de execução
- Fallback automático para CPU se a GPU não estiver disponível
- Divisão de grandes intervalos de busca em blocos menores para melhor compatibilidade com GPU
- Estimativas de performance com GPU

## Solução de Problemas Comuns

### Nenhum dispositivo OpenCL encontrado

Se o programa não detectar dispositivos OpenCL:

1. Certifique-se de que seus drivers de GPU estão atualizados
2. Verifique se o OpenCL está instalado junto com seus drivers
3. Em algumas placas NVIDIA, a instalação completa dos drivers CUDA pode ser necessária

### Aviso "Valor muito grande para GPU"

Este aviso é normal quando se tenta resolver puzzles de alta dificuldade (por exemplo, 71+ bits). 
O programa irá dividir automaticamente o trabalho em blocos menores, ou usar a CPU para valores 
que excedam os limites da GPU.

### Mensagem "Warning: Failed to load kernel source"

Esta mensagem pode aparecer se o programa não encontrar o arquivo `crypto_kernels.cl`. 
Isso não é um problema crítico, pois o programa utilizará um kernel embutido como fallback.

## Implementação Técnica

Nossa implementação OpenCL:

1. Detecta automaticamente dispositivos OpenCL disponíveis
2. Permite ao usuário escolher qual GPU usar
3. Carrega dinamicamente a biblioteca OpenCL em tempo de execução
4. Divide o espaço de busca em pedaços menores para processamento paralelo
5. Implementa kernels otimizados para SHA256 e RIPEMD160
6. Fornece fallback para CPU se ocorrerem erros na GPU

## Suporte Multi-GPU

Atualmente, o programa suporta a seleção de uma única GPU. O suporte para múltiplas GPUs
está planejado para futuras versões.

## Tamanho de Batch Otimizado

O programa calcula automaticamente um tamanho de batch otimizado para sua GPU. Isto permite:

1. Melhor utilização de recursos da GPU
2. Evitar estouros de memória em GPUs com memória limitada
3. Balancear a carga entre CPU e GPU quando necessário

## Contribuindo

Se você encontrar problemas ou tiver ideias para melhorar o suporte a GPU, por favor:

1. Abra uma issue descrevendo o problema
2. Forneça detalhes do seu hardware e sistema operacional
3. Se possível, inclua logs de erro ou saída do programa 