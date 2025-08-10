#!/bin/bash
# lifetime_audit.sh

echo "=== TABBY LIFETIME AUDIT ==="
echo "Дата: $(date)"
echo

echo "1. АВТОГЕНЕРИРОВАННЫЕ LIFETIMES ('life0, 'life1, etc.):"
grep -rn "'life[0-9]" --include="*.rs" . | head -20

echo -e "\n2. ЯВНЫЕ LIFETIMES ('a, 'b, etc.):"
grep -rn "<'[a-zA-Z_][a-zA-Z0-9_]*>" --include="*.rs" . | head -20

echo -e "\n3. ANONYMOUS LIFETIMES ('_):"
grep -rn "'\s*_" --include="*.rs" . | head -20

echo -e "\n4. СТРУКТУРЫ С LIFETIMES:"
grep -rn "struct.*<.*'" --include="*.rs" . | head -10

echo -e "\n5. СТАТИСТИКА ПО ФАЙЛАМ:"
find . -name "*.rs" -exec grep -l "'" {} \; | wc -l
echo "файлов содержат lifetimes"