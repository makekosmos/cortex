import { describe, expect, it } from 'vitest';
import { HLC } from '../hlc';

describe('HLC', () => {
  it('tick advances time', () => {
    const past = new HLC('2020-01-01T00:00:00.000Z', 5, 'device-a');
    const ticked = past.tick();

    expect(ticked.wallTime > past.wallTime).toBe(true);
    expect(ticked.counter).toBe(0);
    expect(ticked.deviceId).toBe('device-a');
  });

  it('tick increments counter when clock unchanged', () => {
    const future = '2099-01-01T00:00:00.000Z';
    const hlc = new HLC(future, 3, 'device-a');
    const ticked = hlc.tick();

    expect(ticked.wallTime).toBe(future);
    expect(ticked.counter).toBe(4);
    expect(ticked.deviceId).toBe('device-a');
  });

  it('merge with remote ahead', () => {
    const local = new HLC('2020-01-01T00:00:00.000Z', 0, 'device-a');
    const remote = new HLC('2099-01-01T00:00:00.000Z', 10, 'device-b');
    const merged = local.merge(remote);

    expect(merged.wallTime).toBe('2099-01-01T00:00:00.000Z');
    expect(merged.counter).toBe(11);
    expect(merged.deviceId).toBe('device-a');
  });

  it('merge with remote behind', () => {
    const local = new HLC('2099-01-01T00:00:00.000Z', 5, 'device-a');
    const remote = new HLC('2020-01-01T00:00:00.000Z', 10, 'device-b');
    const merged = local.merge(remote);

    expect(merged.wallTime).toBe('2099-01-01T00:00:00.000Z');
    expect(merged.counter).toBe(6);
    expect(merged.deviceId).toBe('device-a');
  });

  it('merge same time different counter', () => {
    const time = '2099-06-15T12:00:00.000Z';
    const local = new HLC(time, 3, 'device-a');
    const remote = new HLC(time, 7, 'device-b');
    const merged = local.merge(remote);

    expect(merged.wallTime).toBe(time);
    expect(merged.counter).toBe(8); // max(3, 7) + 1
    expect(merged.deviceId).toBe('device-a');
  });

  it('serialization roundtrip', () => {
    const original = new HLC('2026-03-28T14:30:00.123Z', 42, 'delphi-web-abc123');
    const str = original.toString();

    expect(str).toBe('2026-03-28T14:30:00.123Z:000042:delphi-web-abc123');

    const parsed = HLC.fromString(str);
    expect(parsed.wallTime).toBe(original.wallTime);
    expect(parsed.counter).toBe(original.counter);
    expect(parsed.deviceId).toBe(original.deviceId);
  });

  it('comparison operators', () => {
    const earlier = new HLC('2020-01-01T00:00:00.000Z', 0, 'device-a');
    const later = new HLC('2025-01-01T00:00:00.000Z', 0, 'device-a');
    const laterHighCounter = new HLC('2025-01-01T00:00:00.000Z', 5, 'device-a');

    expect(earlier.compareTo(later)).toBeLessThan(0);
    expect(later.compareTo(earlier)).toBeGreaterThan(0);
    expect(later.compareTo(laterHighCounter)).toBeLessThan(0);
    expect(laterHighCounter.compareTo(later)).toBeGreaterThan(0);
    expect(earlier.compareTo(earlier)).toBe(0);

    // Static compare works the same
    expect(HLC.compare(earlier, later)).toBeLessThan(0);
  });

  it('device_id tiebreaker', () => {
    const time = '2025-06-15T12:00:00.000Z';
    const a = new HLC(time, 0, 'aaa');
    const b = new HLC(time, 0, 'zzz');

    expect(a.compareTo(b)).toBeLessThan(0);
    expect(b.compareTo(a)).toBeGreaterThan(0);
  });

  it('now creates valid HLC', () => {
    const before = new Date().toISOString();
    const hlc = HLC.now('test-device');
    const after = new Date().toISOString();

    expect(hlc.wallTime >= before).toBe(true);
    expect(hlc.wallTime <= after).toBe(true);
    expect(hlc.counter).toBe(0);
    expect(hlc.deviceId).toBe('test-device');

    // Roundtrip should work
    const str = hlc.toString();
    const parsed = HLC.fromString(str);
    expect(parsed.wallTime).toBe(hlc.wallTime);
    expect(parsed.counter).toBe(0);
    expect(parsed.deviceId).toBe('test-device');
  });
});
