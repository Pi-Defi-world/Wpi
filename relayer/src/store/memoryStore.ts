import { BaseStore, emptyState } from './baseStore.js';

/** In-memory `IdempotencyStore`, for tests and dry-run demos. Nothing survives a restart. */
export class MemoryStore extends BaseStore {
  constructor() {
    super(emptyState());
  }

  protected persist(): void {
    // no-op
  }
}
