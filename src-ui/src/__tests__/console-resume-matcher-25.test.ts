import { describe, expect, it } from 'vitest';
import { isExpiredResumeError } from '../console-chat';

// 续聊失效判定的回归（09-17 编排者核对 fix-25-r1 时发现）：
// 旧写法只看「会话词」与「失效词」是否各自出现在整句里，
// 于是「Invalid API key for this session」「unknown error in thread pool」都被判成会话失效，
// 会清掉有效会话、提示「已失效」、再重派一次多花额度。现在要求两类词贴在一起。
describe('续聊失效判定：普通报错不回退，真失效才回退', () => {
  const notExpired = [
    'HTTP 502 Bad Gateway',
    'Invalid API key for this session',
    'unknown error in thread pool',
    'rate limit exceeded, please retry the conversation later',
    'Error: invalid model name gpt-9',
    'thread panicked at src/main.rs: index out of bounds',
    'network error: connection reset while streaming conversation',
    'Worker thread error: task not found',
    'thread pool: connection not found',
  ];
  for (const message of notExpired) {
    it(`不回退：${message}`, () => {
      expect(isExpiredResumeError(message)).toBe(false);
    });
  }

  const expired = [
    'Error: session not found',
    'No session found with id abc',
    'thread 01a0 not found',
    'conversation does not exist',
    'cannot resume: session expired',
    "session 'abc-123' has expired",
    'session id 42 does not exist',
  ];
  for (const message of expired) {
    it(`应回退：${message}`, () => {
      expect(isExpiredResumeError(message)).toBe(true);
    });
  }
});
