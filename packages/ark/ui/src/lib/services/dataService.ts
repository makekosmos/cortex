/**
 * DataService - Optimized data access layer with caching and batching
 *
 * Features:
 * - LRU cache for page data
 * - IPC request batching (debounce)
 * - Prefetching for smooth scrolling
 * - Request deduplication
 */

import { getPage as apiGetPage, getTotalCount as apiGetTotalCount } from "../api";
import type { Filter, PageRequest, PageResponse } from "../types";

// ============================================================================
// LRU Cache Implementation
// ============================================================================

interface CacheEntry<T> {
    data: T;
    timestamp: number;
    key: string;
}

class LRUCache<T> {
    private cache = new Map<string, CacheEntry<T>>();
    private maxSize: number;
    private ttlMs: number;

    constructor(maxSize = 100, ttlMs = 30000) {
        this.maxSize = maxSize;
        this.ttlMs = ttlMs;
    }

    private makeKey(request: PageRequest): string {
        return JSON.stringify({
            o: request.offset,
            l: request.limit,
            sc: request.sort_column,
            sd: request.sort_direction,
            f: request.filters,
            s: request.search,
        });
    }

    get(request: PageRequest): T | null {
        const key = this.makeKey(request);
        const entry = this.cache.get(key);

        if (!entry) return null;

        // Check TTL
        if (Date.now() - entry.timestamp > this.ttlMs) {
            this.cache.delete(key);
            return null;
        }

        // Move to end (most recently used)
        this.cache.delete(key);
        this.cache.set(key, entry);

        return entry.data;
    }

    set(request: PageRequest, data: T): void {
        const key = this.makeKey(request);

        // Evict oldest if at capacity
        if (this.cache.size >= this.maxSize) {
            const firstKey = this.cache.keys().next().value;
            if (firstKey) this.cache.delete(firstKey);
        }

        this.cache.set(key, {
            data,
            timestamp: Date.now(),
            key,
        });
    }

    invalidate(): void {
        this.cache.clear();
    }

    invalidateByFilter(predicate: (key: string) => boolean): void {
        for (const [key] of this.cache) {
            if (predicate(key)) {
                this.cache.delete(key);
            }
        }
    }

    get size(): number {
        return this.cache.size;
    }
}

// ============================================================================
// Request Deduplication
// ============================================================================

const pendingRequests = new Map<string, Promise<PageResponse>>();

function getRequestKey(request: PageRequest): string {
    return JSON.stringify(request);
}

// ============================================================================
// Data Service
// ============================================================================

class DataService {
    private pageCache = new LRUCache<PageResponse>(200, 60000); // 200 pages, 60s TTL
    private countCache = new Map<string, { count: number; timestamp: number }>();
    private prefetchQueue: PageRequest[] = [];
    private isPrefetching = false;

    /**
     * Get a page of data with caching and deduplication
     */
    async getPage(request: PageRequest): Promise<PageResponse> {
        // Check cache first
        const cached = this.pageCache.get(request);
        if (cached) {
            // Prefetch adjacent pages in background
            this.schedulePrefetch(request);
            return cached;
        }

        // Deduplicate concurrent requests
        const key = getRequestKey(request);
        const pending = pendingRequests.get(key);
        if (pending) {
            return pending;
        }

        // Make the request
        const promise = apiGetPage(request)
            .then((response) => {
                this.pageCache.set(request, response);
                pendingRequests.delete(key);

                // Prefetch adjacent pages
                this.schedulePrefetch(request);

                return response;
            })
            .catch((error) => {
                pendingRequests.delete(key);
                throw error;
            });

        pendingRequests.set(key, promise);
        return promise;
    }

    /**
     * Get total count with short-term caching
     */
    async getTotalCount(filters?: Filter[], search?: string): Promise<number> {
        const cacheKey = JSON.stringify({ f: filters, s: search });
        const cached = this.countCache.get(cacheKey);

        if (cached && Date.now() - cached.timestamp < 5000) {
            return cached.count;
        }

        const count = await apiGetTotalCount(filters, search);
        this.countCache.set(cacheKey, { count, timestamp: Date.now() });
        return count;
    }

    /**
     * Invalidate cache when filters/sort change
     */
    invalidateCache(): void {
        this.pageCache.invalidate();
        this.countCache.clear();
        this.prefetchQueue = [];
    }

    /**
     * Schedule prefetch of adjacent pages
     */
    private schedulePrefetch(currentRequest: PageRequest): void {
        const { offset, limit } = currentRequest;

        // Prefetch next and previous pages
        const pagesToPrefetch = [
            { ...currentRequest, offset: offset + limit }, // Next page
            { ...currentRequest, offset: Math.max(0, offset - limit) }, // Previous page
            { ...currentRequest, offset: offset + limit * 2 }, // Page after next
        ];

        for (const request of pagesToPrefetch) {
            if (request.offset >= 0 && !this.pageCache.get(request)) {
                this.prefetchQueue.push(request);
            }
        }

        this.processPrefetchQueue();
    }

    /**
     * Process prefetch queue with idle callback
     */
    private processPrefetchQueue(): void {
        if (this.isPrefetching || this.prefetchQueue.length === 0) return;

        this.isPrefetching = true;

        const processNext = () => {
            const request = this.prefetchQueue.shift();
            if (!request) {
                this.isPrefetching = false;
                return;
            }

            // Skip if already cached
            if (this.pageCache.get(request)) {
                requestIdleCallback(processNext);
                return;
            }

            // Low priority fetch
            apiGetPage(request)
                .then((response) => {
                    this.pageCache.set(request, response);
                })
                .catch(() => {
                    // Ignore prefetch errors
                })
                .finally(() => {
                    requestIdleCallback(processNext);
                });
        };

        requestIdleCallback(processNext);
    }

    /**
     * Get cache statistics for debugging
     */
    getCacheStats(): { pageCount: number; countCacheSize: number } {
        return {
            pageCount: this.pageCache.size,
            countCacheSize: this.countCache.size,
        };
    }
}

// Singleton instance
export const dataService = new DataService();

// ============================================================================
// IPC Batcher - Batch multiple rapid requests
// ============================================================================

interface BatchedRequest<T> {
    resolve: (value: T) => void;
    reject: (error: Error) => void;
    request: PageRequest;
}

class IPCBatcher {
    private batchQueue: BatchedRequest<PageResponse>[] = [];
    private batchTimeout: ReturnType<typeof setTimeout> | null = null;
    private batchDelay = 16; // ~1 frame at 60fps

    /**
     * Add request to batch queue
     */
    async request(pageRequest: PageRequest): Promise<PageResponse> {
        return new Promise((resolve, reject) => {
            this.batchQueue.push({ resolve, reject, request: pageRequest });
            this.scheduleBatch();
        });
    }

    private scheduleBatch(): void {
        if (this.batchTimeout) return;

        this.batchTimeout = setTimeout(() => {
            this.processBatch();
        }, this.batchDelay);
    }

    private async processBatch(): Promise<void> {
        this.batchTimeout = null;
        const batch = this.batchQueue.splice(0);

        if (batch.length === 0) return;

        // Deduplicate requests in batch
        const uniqueRequests = new Map<string, BatchedRequest<PageResponse>[]>();

        for (const item of batch) {
            const key = JSON.stringify(item.request);
            const existing = uniqueRequests.get(key) || [];
            existing.push(item);
            uniqueRequests.set(key, existing);
        }

        // Process unique requests
        for (const [, items] of uniqueRequests) {
            const request = items[0].request;

            try {
                const response = await dataService.getPage(request);
                for (const item of items) {
                    item.resolve(response);
                }
            } catch (error) {
                for (const item of items) {
                    item.reject(error as Error);
                }
            }
        }
    }
}

export const ipcBatcher = new IPCBatcher();

// ============================================================================
// Utility: requestIdleCallback polyfill
// ============================================================================

interface WindowWithIdleCallback {
    requestIdleCallback?: (callback: () => void) => number;
}

const requestIdleCallback =
    (window as WindowWithIdleCallback).requestIdleCallback ||
    ((cb: () => void) => setTimeout(cb, 1));
