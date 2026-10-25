import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// 开发端口要与 tauri.conf.json 的 devUrl 一致，被占用时直接报错，不换端口。
// 构建目标对齐 Windows 的 WebView2 与 macOS 的 WKWebView。
export default defineConfig({
  plugins: [react()],
  server: { port: 5173, strictPort: true },
  build: {
    outDir: 'dist',
    target: ['es2021', 'chrome105', 'safari15'],
    rollupOptions: {
      output: {
        // React 体积大、很少变，单独一组；其余依赖归 vendor，应用代码改动时这两组不用重下。
        codeSplitting: {
          groups: [
            { name: 'react', test: /node_modules[\\/](react|react-dom|scheduler)[\\/]/ },
            { name: 'vendor', test: /node_modules/ },
          ],
        },
      },
    },
  },
});
