# Ark Core - Test Results

**Date:** 2026-01-19  
**Schema Version:** 2.0.0  
**Python:** 3.10.11

---

## Unit Tests

| Metric | Value |
|--------|-------|
| **Total Tests** | 56 |
| **Passed** | 56 |
| **Failed** | 0 |
| **Subtests Passed** | 3 |
| **Execution Time** | 2.00s |

### Test Categories

| Category | Tests | Status |
|----------|-------|--------|
| TestUtilityFunctions | 8 | PASSED |
| TestArk | 37 | PASSED |
| TestSchemaIntegrity | 5 | PASSED |
| TestDataclasses | 3 | PASSED |

---

## Code Coverage

| File | Statements | Missed | Coverage |
|------|-----------|--------|----------|
| ark.py | 301 | 57 | **81%** |

### Uncovered Lines
- CLI interface (lines 790-830, 834)
- Edge cases in date parsing (lines 117, 175, 192-194)
- Some error handling paths

---

## Code Quality

| Tool | Status | Issues |
|------|--------|--------|
| **Ruff** (linter) | PASSED | 0 errors |
| **mypy** (type checker) | PASSED | 0 errors (strict mode) |

---

## Performance Tests

| Operation | Events | Time | Rate |
|-----------|--------|------|------|
| Bulk insert | 10,000 | 1.26s | **7,920 events/sec** |
| Query (limit 1000) | - | 16.2ms | - |
| Filtered query (limit 500) | - | 5.3ms | - |
| Full-text search | - | 3.0ms | - |
| Count all | 10,001 | 3.1ms | - |
| Daily stats | - | 6.3ms | - |

### Storage
| Metric | Value |
|--------|-------|
| File size (10K events) | 4.63 MB |
| Estimated 50yr (30M events) | ~14 GB |

---

## Database Integrity

| Check | Result |
|-------|--------|
| **Integrity check** | PASSED |
| **Journal mode** | WAL |
| **Schema version** | 2.0.0 |
| **FTS5 index** | EXISTS |

### Schema Objects

| Object Type | Count |
|-------------|-------|
| Tables | 7 (+ 4 FTS internal) |
| Indexes | 20 |
| Triggers | 6 |
| Views | 2 |

### Tables
- `events` - Main event storage
- `entities` - Reference entities
- `event_entity_links` - Event-entity relationships
- `embeddings` - Vector embeddings
- `imports` - Import tracking
- `metadata` - Schema version
- `events_fts` - Full-text search (virtual)

---

## Summary

| Aspect | Status |
|--------|--------|
| Unit Tests | **56/56 PASSED** |
| Coverage | **81%** |
| Linting | **CLEAN** |
| Type Safety | **100%** (mypy strict) |
| Performance | **7,920 events/sec** |
| Database Integrity | **VERIFIED** |

**All checks passed.**
