import { describe, expect, it } from 'vitest';
import { downloadProgressText, popoverStatus, triggerView, type AppUpdateModel } from '../app-update-view';

function baseModel(overrides: Partial<AppUpdateModel> = {}): AppUpdateModel {
  return {
    current: '0.3.11',
    available: null,
    checking: false,
    checked: false,
    checkError: '',
    progress: null,
    installError: '',
    ...overrides,
  };
}

describe('顶栏版本与更新入口（TASK-88）', () => {
  it('未拿到版本号且无新版时不渲染按钮', () => {
    expect(triggerView(baseModel({ current: '' }))).toEqual({
      visible: false,
      primary: false,
      label: '',
      title: '',
    });
  });

  it('无新版或启动检查失败时，顶栏安静显示当前版本号', () => {
    expect(triggerView(baseModel())).toEqual({
      visible: true,
      primary: false,
      label: 'v0.3.11',
      title: '软件版本与更新',
    });
    expect(triggerView(baseModel({ checked: true, checkError: '检查更新失败：网络不可达' }))).toEqual({
      visible: true,
      primary: false,
      label: 'v0.3.11',
      title: '软件版本与更新',
    });
  });

  it('检测到新版、下载中、安装失败时顶栏切换为主按钮', () => {
    const available = { version: '0.3.12', notes: '- 修复问题' };
    expect(triggerView(baseModel({ available }))).toEqual({
      visible: true,
      primary: true,
      label: '更新到 0.3.12',
      title: '当前版本 0.3.11',
    });
    expect(triggerView(baseModel({ available, progress: '下载中 42%' }))).toEqual({
      visible: true,
      primary: true,
      label: '下载中 42%',
      title: '当前版本 0.3.11',
    });
    expect(triggerView(baseModel({ available, installError: '更新失败：校验不通过' }))).toEqual({
      visible: true,
      primary: true,
      label: '更新失败，重试',
      title: '更新失败：校验不通过',
    });
  });

  it('浮层状态区分检查中、已是最新、检查失败与安装错误', () => {
    expect(popoverStatus(baseModel())).toEqual({ text: '未检查更新', alert: false });
    expect(popoverStatus(baseModel({ checking: true }))).toEqual({ text: '正在检查更新…', alert: false });
    expect(popoverStatus(baseModel({ checked: true }))).toEqual({ text: '已是最新版本', alert: false });
    expect(popoverStatus(baseModel({ checked: true, checkError: '检查更新失败：超时' }))).toEqual({
      text: '检查更新失败：超时',
      alert: true,
    });

    const available = { version: '0.3.12', notes: '' };
    expect(popoverStatus(baseModel({ available, checked: true }))).toEqual({ text: '', alert: false });
    expect(popoverStatus(baseModel({ available, progress: '准备下载…' }))).toEqual({
      text: '准备下载…',
      alert: false,
    });
    expect(popoverStatus(baseModel({ available, installError: '更新失败：权限不足' }))).toEqual({
      text: '更新失败：权限不足',
      alert: true,
    });
  });

  it('下载进度在总大小已知时算百分比，未知时只显示下载中', () => {
    expect(downloadProgressText(50, 200)).toBe('下载中 25%');
    expect(downloadProgressText(250, 200)).toBe('下载中 100%');
    expect(downloadProgressText(50, null)).toBe('下载中…');
    expect(downloadProgressText(50, 0)).toBe('下载中…');
  });
});
