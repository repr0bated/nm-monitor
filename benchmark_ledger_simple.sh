#!/bin/bash
# Simple shell benchmark to demonstrate ledger file I/O improvement

echo "🔬 Ledger File I/O Benchmark"
echo ""
echo "Testing with 1,000 writes..."
echo ""

iterations=1000

# Benchmark OLD approach (open/close every write)
echo -n "⏱️  Old approach (open/close): "
rm -f /tmp/bench_old.txt
start_old=$(date +%s%N)
for i in $(seq 1 $iterations); do
    # Simulates: OpenOptions::new().append(true).open()
    echo "Block $i data" >> /tmp/bench_old.txt
done
end_old=$(date +%s%N)
old_ms=$(( (end_old - start_old) / 1000000 ))
echo "${old_ms}ms"

# Benchmark NEW approach (keep fd open)
echo -n "⚡ New approach (persistent fd): "
rm -f /tmp/bench_new.txt
start_new=$(date +%s%N)
# Simulates: BufWriter keeps file handle open
exec 3>> /tmp/bench_new.txt
for i in $(seq 1 $iterations); do
    echo "Block $i data" >&3
done
exec 3>&-
end_new=$(date +%s%N)
new_ms=$(( (end_new - start_new) / 1000000 ))
echo "${new_ms}ms"

# Calculate improvement
speedup=$(echo "scale=1; $old_ms / $new_ms" | bc -l)
improvement=$(echo "scale=0; ($old_ms - $new_ms) * 100 / $old_ms" | bc -l)

echo ""
echo "📊 Results:"
echo "  • Speedup:     ${speedup}x faster"
echo "  • Improvement: ${improvement}% reduction in time"
echo ""
echo "💾 Syscalls (estimated):"
echo "  • Old: ~$((iterations * 4)) syscalls (open, lseek, write, close × $iterations)"
echo "  • New: ~$((iterations + 2)) syscalls (open, write × $iterations, close)"
echo "  • Reduction: ~$((100 - (iterations + 2) * 100 / (iterations * 4)))%"
echo ""
echo "✅ This is why we keep the file handle open!"
echo ""

# Cleanup
rm -f /tmp/bench_old.txt /tmp/bench_new.txt
