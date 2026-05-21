# todoFilterService TS baseline benchmark

Runs per case: 200 (после 20 warmup)

## n = 1000 todos

| benchmark             | median ms | p99 ms |   items/sec |
| --------------------- | --------: | -----: | ----------: |
| filterTodos(inbox)    |     0.028 |  0.082 |  35 335 689 |
| filterTodos(today)    |     0.065 |  0.217 |  15 360 983 |
| filterTodos(upcoming) |     0.063 |  0.267 |  15 923 567 |
| filterTodos(anytime)  |     0.032 |  0.189 |  31 152 648 |
| filterTodos(someday)  |     0.007 |  0.029 | 147 058 824 |
| filterTodos(logbook)  |     0.060 |  0.105 |  16 722 408 |
| filterTodos(trash)    |     0.050 |  0.090 |  19 841 270 |
| countAll              |     0.192 |  0.600 |   5 200 208 |

## n = 10000 todos

| benchmark             | median ms | p99 ms |   items/sec |
| --------------------- | --------: | -----: | ----------: |
| filterTodos(inbox)    |     0.149 |  1.247 |  67 294 751 |
| filterTodos(today)    |     0.576 |  1.118 |  17 373 176 |
| filterTodos(upcoming) |     0.834 |  1.363 |  11 987 533 |
| filterTodos(anytime)  |     0.189 |  0.359 |  52 910 053 |
| filterTodos(someday)  |     0.071 |  0.230 | 140 845 070 |
| filterTodos(logbook)  |     0.822 |  1.340 |  12 158 055 |
| filterTodos(trash)    |     0.807 |  1.477 |  12 390 038 |
| countAll              |     1.648 |  3.526 |   6 066 489 |

## Summary

Используется в evidence.md как TS baseline. Rust criterion bench
должен показать сравнимые или лучшие numbers на той же synthetic data.
