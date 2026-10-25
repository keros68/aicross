// 首帧前就要知道是不是 macOS：那里用系统的红绿灯按钮与圆角，不画自绘窗口按钮。
export const IS_MACOS: boolean = typeof navigator !== 'undefined' && /Macintosh|Mac OS X/.test(navigator.userAgent);
