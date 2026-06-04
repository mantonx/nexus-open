# RPC Design Decision: net/rpc vs gRPC

**Decision:** Use Go's `net/rpc` for module communication in v2.0

**Date:** 2025-10-12

---

## Context

Nexus Open v2.0 uses a plugin architecture where modules (CPU, GPU, Weather, etc.) run as separate processes and communicate with the host via RPC. We needed to choose between:

1. **net/rpc** - Go's standard library RPC
2. **gRPC** - Google's high-performance RPC framework

---

## Decision

**Chose net/rpc** for the following reasons:

### 1. Simplicity
- No build dependencies (protoc, plugins)
- No code generation required
- ~100 lines of code vs ~300+ with gRPC
- 5-minute implementation vs 30+ minutes

### 2. Go-Native Integration
- Uses `encoding/gob` (Go binary encoding)
- Type-safe at compile time
- Perfect for Go-to-Go communication
- Easy debugging (plain Go structs)

### 3. Sufficient Performance
Our workload characteristics:
- Call frequency: 1-5 seconds per module
- Payload size: ~100 bytes (Primary, Secondary, Spark)
- Total RPC calls: ~2-8 per second
- **Conclusion:** net/rpc overhead is negligible

### 4. Faster Development
- Immediate productivity
- No learning curve for contributors
- Fewer moving parts to debug
- Smaller binary size

---

## Trade-offs Accepted

### What We Give Up with net/rpc:

1. **Multi-language modules**
   - net/rpc is Go-only
   - Cannot write modules in Python, Rust, etc.
   - **Mitigation:** V2.0 focuses on Go modules, multi-language in v3.0+

2. **Schema evolution**
   - Struct changes can break compatibility
   - No built-in versioning
   - **Mitigation:** Semantic versioning + careful API design

3. **Advanced features**
   - No streaming (one request → one response)
   - No bidirectional communication
   - **Mitigation:** Not needed for current use case

---

## Performance Comparison

### Benchmark (hypothetical):
```
net/rpc:  Sample() call = ~1-2ms
gRPC:     Sample() call = ~0.5-1ms

Difference: ~1ms per call
Impact: 1ms × 8 modules = 8ms/second
Verdict: Negligible for our use case
```

### Why gRPC Would Be Faster:
- Protobuf encoding is more efficient than gob
- HTTP/2 multiplexing
- Optimized C implementation

### Why It Doesn't Matter:
- We call modules every 1-5 seconds, not 1000x/sec
- Payload is tiny (~100 bytes)
- Network latency > encoding overhead (same process)

---

## Comparison Matrix

| Aspect | net/rpc | gRPC |
|--------|---------|------|
| **Implementation** | ✅ 100 LOC | ⚠️ 300+ LOC + generated |
| **Build** | ✅ Zero deps | ❌ protoc, plugins |
| **Multi-language** | ❌ Go only | ✅ Any language |
| **Performance** | ✅ Good enough | ✅ Excellent |
| **Debugging** | ✅ Easy | ⚠️ Moderate |
| **Binary size** | ✅ Small | ⚠️ Larger |
| **Streaming** | ❌ | ✅ |
| **Learning curve** | ✅ Minimal | ⚠️ Moderate |

---

## Future Migration Path

### When to Consider gRPC:

1. **Community demand for multi-language modules**
   - Users want to write modules in Python, Rust, etc.
   - Example: ML-based modules in Python

2. **Streaming requirements**
   - Real-time sensor data (>1Hz updates)
   - Live graph updates
   - Bidirectional communication

3. **Performance bottleneck**
   - Profiling shows RPC is a bottleneck
   - High-frequency modules (>10Hz)
   - Large payload sizes (>1KB)

### Migration Strategy:

**go-plugin supports both protocols simultaneously:**
```go
client := plugin.NewClient(&plugin.ClientConfig{
    AllowedProtocols: []plugin.Protocol{
        plugin.ProtocolNetRPC,  // Current
        plugin.ProtocolGRPC,    // Add later
    },
})
```

**Steps:**
1. Add `.proto` definitions
2. Generate gRPC code
3. Implement gRPC plugin alongside net/rpc
4. Deprecate net/rpc over 2-3 releases
5. Remove net/rpc code

**Timeline:** Earliest in v2.2+ (6+ months after v2.0 release)

---

## References

- [HashiCorp go-plugin documentation](https://github.com/hashicorp/go-plugin)
- [Go net/rpc package](https://pkg.go.dev/net/rpc)
- [gRPC Go documentation](https://grpc.io/docs/languages/go/)
- [Protocol Buffers](https://protobuf.dev/)

---

## Approval

**Reviewed by:** Development team
**Status:** Approved for v2.0
**Revisit:** v2.2 planning (after v2.0 release + community feedback)
