import { searchFatSecret } from '../fatsecret';

// Sample HTML that mimics FatSecret search results page structure
const SAMPLE_HTML = `
<html>
<body>
<table>
  <tr>
    <td>
      <a class="prominent" href="/калории-питание/общий/куриная-грудка">Куриная Грудка</a>
      &nbsp;
      <a class="brand" href="/brand/123">(Мираторг)</a>
      <div class="smallText greyText">в 100 г - Калории: 165ккал | Жир: 3,60г | Углев: 0,00г | Белк: 31,00г</div>
    </td>
  </tr>
  <tr>
    <td>
      <a class="prominent" href="/калории-питание/общий/рис-белый">Рис Белый</a>
      <div class="smallText greyText">в 100 г - Калории: 130ккал | Жир: 0,28г | Углев: 28,20г | Белк: 2,69г</div>
    </td>
  </tr>
  <tr>
    <td>
      <a class="prominent" href="/калории-питание/общий/молоко">Молоко 3,2%</a>
      &nbsp;
      <a class="brand" href="/brand/456">(Простоквашино)</a>
      <div class="smallText greyText">в 1 стакан - Калории: 150ккал | Жир: 8,00г | Углев: 12,00г | Белк: 8,00г</div>
    </td>
  </tr>
</table>
</body>
</html>
`;

// HTML with no valid results (missing calorie info)
const EMPTY_HTML = `
<html><body>
  <a class="prominent" href="/test">Test Food</a>
  <div class="smallText greyText">нет данных о калориях</div>
</body></html>
`;

// HTML with edge-case number formats
const EDGE_CASE_HTML = `
<html><body>
<table>
  <tr>
    <td>
      <a class="prominent" href="/калории-питание/общий/масло">Сливочное Масло</a>
      <div class="smallText greyText">в 100 г - Калории: 717ккал | Жир: 81,11г | Углев: 0,06г | Белк: 0,85г</div>
    </td>
  </tr>
</table>
</body></html>
`;

// HTML with HTML entities in name
const ENTITIES_HTML = `
<html><body>
<table>
  <tr>
    <td>
      <a class="prominent" href="/test/item">Сыр&#160;&amp;&#160;Крекеры</a>
      <div class="smallText greyText">в 100 г - Калории: 350ккал | Жир: 20,00г | Углев: 25,00г | Белк: 15,00г</div>
    </td>
  </tr>
</table>
</body></html>
`;

beforeEach(() => {
  jest.resetAllMocks();
});

function mockFetch(html: string, ok = true) {
  (globalThis as Record<string, unknown>).fetch = jest.fn().mockResolvedValue({
    ok,
    text: () => Promise.resolve(html),
  });
}

describe('searchFatSecret', () => {
  it('parses multiple results from search page HTML', async () => {
    mockFetch(SAMPLE_HTML);
    const results = await searchFatSecret('курица');

    expect(results).toHaveLength(3);

    // First result: Куриная Грудка (Мираторг)
    expect(results[0].name).toBe('Куриная Грудка');
    expect(results[0].brand).toBe('Мираторг');
    expect(results[0].macros.calories).toBe(165);
    expect(results[0].macros.protein).toBe(31);
    expect(results[0].macros.fat).toBe(3.6);
    expect(results[0].macros.carbs).toBe(0);
    expect(results[0].servingSize).toBe(100);
    expect(results[0].servingUnit).toBe('г');

    // Second result: no brand
    expect(results[1].name).toBe('Рис Белый');
    expect(results[1].brand).toBeUndefined();
    expect(results[1].macros.calories).toBe(130);
    expect(results[1].macros.carbs).toBe(28.2);
  });

  it('parses serving size other than 100g', async () => {
    mockFetch(SAMPLE_HTML);
    const results = await searchFatSecret('молоко');

    const milk = results[2];
    expect(milk.servingSize).toBe(1);
    expect(milk.servingUnit).toBe('стакан');
  });

  it('returns empty array when no calorie data found', async () => {
    mockFetch(EMPTY_HTML);
    const results = await searchFatSecret('nothing');
    expect(results).toEqual([]);
  });

  it('returns empty array on HTTP error', async () => {
    mockFetch('', false);
    const results = await searchFatSecret('test');
    expect(results).toEqual([]);
  });

  it('returns empty array on network failure', async () => {
    (globalThis as Record<string, unknown>).fetch = jest.fn().mockRejectedValue(new Error('Network error'));
    const results = await searchFatSecret('test');
    expect(results).toEqual([]);
  });

  it('parses high-value macros correctly', async () => {
    mockFetch(EDGE_CASE_HTML);
    const results = await searchFatSecret('масло');

    expect(results).toHaveLength(1);
    expect(results[0].macros.calories).toBe(717);
    expect(results[0].macros.fat).toBe(81.1);
    expect(results[0].macros.carbs).toBe(0.1);
    expect(results[0].macros.protein).toBe(0.9);
  });

  it('decodes HTML entities in food names', async () => {
    mockFetch(ENTITIES_HTML);
    const results = await searchFatSecret('сыр');

    expect(results).toHaveLength(1);
    expect(results[0].name).toBe('Сыр & Крекеры');
  });

  it('generates ID with fs- prefix from href', async () => {
    mockFetch(SAMPLE_HTML);
    const results = await searchFatSecret('test');

    expect(results[0].id).toMatch(/^fs-/);
    expect(results[0].id).toContain(encodeURIComponent('/калории-питание/общий/куриная-грудка'));
  });

  it('passes page parameter correctly', async () => {
    mockFetch(SAMPLE_HTML);
    await searchFatSecret('test', 3);

    const calledUrl = (globalThis.fetch as jest.Mock).mock.calls[0][0];
    expect(calledUrl).toContain('pg=2'); // page 3 → pg=2 (0-indexed)
  });
});
