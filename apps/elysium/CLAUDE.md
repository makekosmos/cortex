# Elysium

Nutrition tracker app — part of the SelfSuite ecosystem. Built with Expo (SDK 54), React Native, TypeScript.

## Architecture

### Stack
- **Framework**: Expo Router (file-based routing)
- **State**: Zustand stores with SQLite persistence (`expo-sqlite`)
- **Styling**: StyleSheet (dark theme, `src/theme/`)
- **Fonts**: Geist family (Regular, SemiBold, Bold, Mono, MonoBold)
- **Food APIs**: FatSecret (HTML scraping, default) + Open Food Facts (V1 CGI)

### Key Directories
```
app/                    # Expo Router pages
  (tabs)/               # Tab screens: diary (index), water, profile
  add-food.tsx          # Food search + add flow
  scanner.tsx           # Barcode scanner
  stats.tsx             # Nutrition statistics
  water-stats.tsx       # Water statistics
  settings.tsx          # App settings
src/
  api/                  # External data sources
    fatsecret.ts        # HTML scraper for fatsecret.ru (HTTP, no auth)
    open-food-facts.ts  # OFF V1 CGI search + barcode lookup
  components/           # Reusable UI components
    WeekChart.tsx        # Period chart (week/month/year) with swipe nav
    CalorieHero.tsx      # Arc progress for calories
    MealCard.tsx         # Meal entry card with edit/delete
    QuantityPicker.tsx   # Quantity selector (servings/grams)
    Card.tsx, CardRow.tsx, StatCard.tsx, MacroCard.tsx
  db/
    database.ts         # SQLite schema, CRUD helpers, KV settings
  stores/               # Zustand stores (hydrate from SQLite on app start)
    nutrition-store.ts   # Meal entries + goals
    water-store.ts       # Water entries + goal
    food-store.ts        # Custom foods + recent foods
    settings-store.ts    # Food source preference
  theme/                # Colors, spacing, fonts, radius
  types/nutrition.ts    # TypeScript interfaces (FoodItem, MealEntry, Macros)
  utils/macros.ts       # Calorie/macro math + formatting
```

### Data Flow
1. Stores call `hydrate()` on app start (in `_layout.tsx`)
2. Reads all data from SQLite into Zustand state
3. Mutations write to SQLite synchronously, then update Zustand
4. Components subscribe to Zustand state reactively

### Database Schema (SQLite)
- `nutrition_entries` — id, date, food_json, quantity, meal_type, logged_at
- `water_entries` — id, date, amount, time
- `custom_foods` — id, name, brand, serving_size, serving_unit, macros_json
- `recent_foods` — id, food_json, used_at
- `settings` — key/value store (goals, water_goal, food_source)

## Conventions
- Language in UI: Russian
- Package manager: bun (bun.lock)
- Build: EAS Build (`eas.json`, profile "preview" for APK)
- Path aliases: `@/` maps to `src/`
- Macros display with one decimal: `12.5г`
- All colors defined in `src/theme/colors.ts`

## Team Agents

Use `/agents` to invoke specialized agents for this project:

- **perf** — Performance optimization expert. Finds bottlenecks, fixes unnecessary re-renders, implements caching, optimizes SQLite queries and list rendering.
- **test** — Testing expert. Creates unit tests, integration tests, E2E tests. Covers stores, API parsers, components, and database layer.
