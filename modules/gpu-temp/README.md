# GPU Temperature Module

Monitors NVIDIA GPU temperature using `nvidia-smi`.

## Requirements

- NVIDIA GPU with drivers installed
- `nvidia-smi` command available in PATH

## Building

```bash
cd modules/gpu-temp
go mod init nexus-gpu-temp
go mod edit -replace nexus-open=../..
go mod tidy
go build -o gpu-temp .
```

## Testing

```bash
# Test standalone (requires nvidia-smi)
nvidia-smi --query-gpu=temperature.gpu --format=csv,noheader,nounits

# Test via host
nexus-open module test exec:./modules/gpu-temp/gpu-temp
```

## Configuration

```yaml
zones:
  - id: gpu
    width: 160
    module: exec:./modules/gpu-temp/gpu-temp
    refresh_ms: 2000
    align: center
    theme_override:
      accent: "#FF6B6B"  # Red accent for GPU
```

## Features

- **Real-time temperature**: Updates every 2 seconds
- **Sparkline history**: Last 60 samples (2 minutes)
- **Severity levels**:
  - OK: <75°C (accent color)
  - Warning: 75-89°C (yellow)
  - Critical: ≥90°C (red)
- **Graceful degradation**: Shows "No GPU" if nvidia-smi fails

## Output Format

- **Primary**: Temperature (e.g., "68°C")
- **Secondary**: "GPU Temp"
- **Sparkline**: Normalized 0-100°C range
- **Icon**: microchip

## Multi-GPU Support

Currently monitors the first GPU only. Future enhancement: cycle through GPUs or show average.

## Troubleshooting

**"No GPU" displayed:**
- Check if NVIDIA drivers are installed: `nvidia-smi`
- Verify GPU is recognized: `lspci | grep -i nvidia`
- Ensure nvidia-smi is in PATH

**Permission errors:**
- nvidia-smi typically doesn't require sudo
- If it does, add to sudoers or fix driver installation

## Example Output

```
Primary:   "72°C"
Secondary: "GPU Temp"
Severity:  "ok"
Sparkline: [0.65, 0.68, 0.70, 0.72, ...]
```
