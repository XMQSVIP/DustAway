import { afterEach, describe, expect, it, vi } from 'vitest';
import { CommandCoordinator, COMMAND_TIMEOUT, READ_TIMEOUT } from '../coordinator';
import type { CommandRequest, ControlInfo, RuntimeSnapshot } from '../types';

const control = (
  revision = '1',
  last_request: ControlInfo['last_request'] = null,
): ControlInfo => ({ runtime_id: 'runtime', revision, last_request });
const snapshot = (info = control()) =>
  ({
    control: info,
    status: {
      id: 'session',
      state: 'ready',
      current_rule: '',
      visited: 0,
      completed: 0,
      total: 0,
      error: null,
    },
    summary: { session_id: 'session', state: 'ready', rules: [] },
    report: null,
  }) as RuntimeSnapshot;
afterEach(() => vi.useRealTimers());
describe('command coordination', () => {
  it('recovers a never-resolving execution from its receipt and ignores its late reply', async () => {
    vi.useFakeTimers();
    let request: CommandRequest;
    let resolveLate!: (v: ControlInfo) => void;
    const invoke = vi.fn().mockImplementation((command, args) => {
      if (command === 'execute_cleanup') {
        request = args.request;
        return new Promise((resolve) => {
          resolveLate = resolve;
        });
      }
      if (args.request)
        return Promise.resolve(
          snapshot(
            control('2', {
              request_id: request.request_id,
              command: 'execute_cleanup',
              value: { scopes: ['user'] },
              error: null,
            }),
          ),
        );
      return Promise.resolve(snapshot());
    });
    const c = new CommandCoordinator(invoke);
    await c.attach();
    const result = c.submit('execute_cleanup');
    await vi.advanceTimersByTimeAsync(COMMAND_TIMEOUT);
    await expect(result).resolves.toEqual({ scopes: ['user'] });
    resolveLate(control('0'));
    await Promise.resolve();
    expect(invoke.mock.calls.filter(([cmd]) => cmd === 'execute_cleanup')).toHaveLength(1);
  });
  it('fences an unaccepted request instead of resending it', async () => {
    vi.useFakeTimers();
    let request: CommandRequest;
    const invoke = vi.fn().mockImplementation((command, args) => {
      if (command === 'start_scan') {
        request = args.request;
        return new Promise(() => {});
      }
      return Promise.resolve(
        snapshot(
          args.request
            ? control('2', {
                request_id: request.request_id,
                command: 'sealed',
                value: null,
                error: { code: 'request_expired', message: '封存' },
              })
            : control(),
        ),
      );
    });
    const c = new CommandCoordinator(invoke);
    await c.attach();
    const result = c.submit('start_scan').catch((e) => e.code);
    await vi.advanceTimersByTimeAsync(COMMAND_TIMEOUT);
    expect(await result).toBe('request_expired');
    expect(invoke.mock.calls.filter(([cmd]) => cmd === 'start_scan')).toHaveLength(1);
  });
  it('blocks queued writes until a failed reconciliation is resolved', async () => {
    vi.useFakeTimers();
    let recovering = false;
    const invoke = vi.fn().mockImplementation((command) => {
      if (command === 'save_settings') {
        recovering = true;
        return new Promise(() => {});
      }
      return recovering ? new Promise(() => {}) : Promise.resolve(snapshot());
    });
    const c = new CommandCoordinator(invoke);
    await c.attach();
    const states: string[] = [];
    c.subscribe((state) => states.push(state));
    const first = c.submit('save_settings').catch((e) => e.code);
    const second = c.submit('save_settings').catch((e) => e.code);
    await vi.advanceTimersByTimeAsync(COMMAND_TIMEOUT + READ_TIMEOUT);
    expect(await first).toBe('response_timeout');
    expect(await second).toBe('state_uncertain');
    expect(states).toEqual(['checking', 'uncertain']);
    expect(invoke.mock.calls.filter(([cmd]) => cmd === 'save_settings')).toHaveLength(1);
  });
  it('bounds a stalled read and permits an explicit retry', async () => {
    vi.useFakeTimers();
    const invoke = vi
      .fn()
      .mockImplementationOnce(() => new Promise(() => {}))
      .mockResolvedValue([]);
    const c = new CommandCoordinator(invoke);
    const result = c.read('get_history').catch((e) => e.code);
    await vi.advanceTimersByTimeAsync(READ_TIMEOUT);
    expect(await result).toBe('response_timeout');
    await expect(c.read('get_history')).resolves.toEqual([]);
  });
});
