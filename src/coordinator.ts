import type { ControlInfo, CommandRequest, RuntimeSnapshot } from './types';

export type CoordinationState = 'ready' | 'checking' | 'uncertain';
export const READ_TIMEOUT = 10_000;
export const COMMAND_TIMEOUT = 15_000;
type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;
export function ipcError(error: unknown): Error & { code?: string } {
  if (typeof error === 'object' && error && 'message' in error)
    return Object.assign(new Error(String(error.message)), {
      code: 'code' in error ? String(error.code) : 'unknown',
    });
  return new Error(String(error));
}
export function withTimeout<T>(promise: Promise<T>, ms: number): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(
      () =>
        reject(
          Object.assign(new Error('等待响应超时，尚不能确认操作结果'), {
            code: 'response_timeout',
          }),
        ),
      ms,
    );
    promise.then(resolve, (e) => reject(ipcError(e))).finally(() => clearTimeout(timer));
  });
}

/** One pending write at a time. Timeout does not cancel or replay the invocation. */
export class CommandCoordinator {
  private control?: ControlInfo;
  private pending?: CommandRequest;
  private tail: Promise<unknown> = Promise.resolve();
  private recovery?: Promise<RuntimeSnapshot>;
  private uncertain = true;
  private listeners = new Set<(checking: CoordinationState) => void>();
  constructor(private invoke: Invoke) {}
  subscribe(listener: (checking: CoordinationState) => void) {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }
  private notify(checking: CoordinationState) {
    this.listeners.forEach((fn) => fn(checking));
  }
  read<T>(command: string, args?: Record<string, unknown>) {
    return withTimeout(this.invoke<T>(command, args), READ_TIMEOUT);
  }
  private accept(control: ControlInfo | undefined) {
    if (!control) throw new Error('后台未提供请求核对信息，请重新加载');
    if (this.control && this.control.runtime_id !== control.runtime_id)
      throw Object.assign(new Error('程序运行实例已变化，请重新加载'), { code: 'runtime_changed' });
    if (!this.control || BigInt(control.revision) >= BigInt(this.control.revision))
      this.control = control;
  }
  async snapshot(scope: 'user' | 'system'): Promise<RuntimeSnapshot> {
    if (scope === 'user' && this.uncertain) return this.reconcile();
    const snapshot = await this.read<RuntimeSnapshot>('get_runtime_snapshot', { scope });
    this.accept(snapshot.control);
    return snapshot;
  }
  attach(): Promise<RuntimeSnapshot> {
    this.control = undefined;
    this.pending = undefined;
    this.uncertain = true;
    return this.reconcile();
  }
  reconcile(): Promise<RuntimeSnapshot> {
    if (this.recovery) return this.recovery;
    this.uncertain = true;
    this.notify('checking');
    this.recovery = this.read<RuntimeSnapshot>('reconcile_task_request', {
      request: this.pending ?? null,
    })
      .then((snapshot) => {
        this.accept(snapshot.control);
        this.uncertain = false;
        this.notify('ready');
        return snapshot;
      })
      .catch((error) => {
        this.notify('uncertain');
        throw error;
      })
      .finally(() => {
        this.recovery = undefined;
      });
    return this.recovery;
  }
  submit<T>(command: string, args?: Record<string, unknown>): Promise<T> {
    const operation = this.tail.then(async () => {
      if (this.uncertain || !this.control)
        throw Object.assign(new Error('暂时无法确认任务状态，请先重新检查'), {
          code: 'state_uncertain',
        });
      const request: CommandRequest = {
        runtime_id: this.control.runtime_id,
        expected_revision: this.control.revision,
        request_id: crypto.randomUUID(),
      };
      this.pending = request;
      let reply: ControlInfo;
      try {
        reply = await withTimeout(
          this.invoke<ControlInfo>(command, { ...args, request }),
          command === 'prepare_cleanup' ? 300_000 : COMMAND_TIMEOUT,
        );
        this.accept(reply);
      } catch {
        // Under the same backend lock this either recovers a receipt or fences
        // an invocation that has not entered yet. Never retry the write.
        const snapshot = await this.reconcile();
        reply = snapshot.control!;
      }
      this.pending = undefined;
      const receipt = reply.last_request;
      if (!receipt || receipt.request_id !== request.request_id)
        throw Object.assign(new Error('原请求已失效，请核对当前结果后重新操作'), {
          code: 'request_expired',
        });
      if (receipt.error) throw ipcError(receipt.error);
      return receipt.value as T;
    });
    this.tail = operation.catch(() => {});
    return operation;
  }
}
