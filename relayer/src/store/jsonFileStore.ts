import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from 'node:fs';
import { dirname } from 'node:path';
import { BaseStore, emptyState, type StoreState } from './baseStore.js';

/**
 * File-backed `IdempotencyStore`. Writes the whole state as JSON to a temp
 * file and renames it into place, so a crash mid-write never leaves a
 * corrupt/partial state file behind.
 */
export class JsonFileStore extends BaseStore {
  constructor(private readonly path: string) {
    super(JsonFileStore.load(path));
  }

  private static load(path: string): StoreState {
    if (!existsSync(path)) return emptyState();
    const raw = readFileSync(path, 'utf8');
    return { ...emptyState(), ...(JSON.parse(raw) as Partial<StoreState>) };
  }

  protected persist(): void {
    mkdirSync(dirname(this.path), { recursive: true });
    const tmpPath = `${this.path}.tmp`;
    writeFileSync(tmpPath, JSON.stringify(this.state, null, 2));
    renameSync(tmpPath, this.path);
  }
}
