/* 共用 Tailwind 設定
   視覺取自使用者提供的 macOS 行事曆截圖（月檢視）：
     淺色 畫布 #f5f5f7、格子 #ffffff、髮絲線 #d8d8dd、今天 #ff3b30
     深色 畫布 #141416、格子 #1e1e20、髮絲線 #333338
   狀態沿用行事曆「事件」的表現：淡色底 + 實心圓點，不用外框、不用圖示。 */
tailwind.config = {
  darkMode: 'class',
  theme: {
    extend: {
      colors: {
        // 淺色（macOS 行事曆 月檢視）
        paper: '#f5f5f7',        // 視窗底、工具列、非本月格子
        surface: '#ffffff',      // 本月格子、面板
        ink: '#1d1d1f',
        muted: '#86868b',
        line: '#d8d8dd',         // 髮絲線
        accent: '#d0271d',       // 行事曆紅（今天、主要動作）
        accentsoft: '#ebebee',   // 分段控制底、hover
        // 深色
        dpaper: '#141416',
        dsurface: '#1e1e20',
        dink: '#f2f2f7',
        dmuted: '#98989d',
        dline: '#333338',
        daccent: '#ff453a',
        daccentsoft: '#2c2c2e',
        // 七個狀態＝七種行事曆：dot 實心圓點、tint 事件底色、dtint 深色版
        st: {
          todo:      { dot: '#8e8e93', tint: '#e9e9eb', dtint: '#303034' },
          proposing: { dot: '#a057d8', tint: '#f0e3fa', dtint: '#332440' },
          parked:    { dot: '#f08c00', tint: '#ffeeda', dtint: '#3d2a10' },
          building:  { dot: '#2f7cf6', tint: '#dde9fd', dtint: '#16283f' },
          review:    { dot: '#e0a500', tint: '#fbf1cf', dtint: '#3a3113' },
          archived:  { dot: '#b0b0b6', tint: '#f2f2f4', dtint: '#232326' },
          done:      { dot: '#2aa84a', tint: '#dcf3e2', dtint: '#16321f' },
        },
      },
      fontFamily: {
        sans: ['-apple-system', 'BlinkMacSystemFont', 'SF Pro Text', 'Inter', 'system-ui', 'PingFang TC', 'Noto Sans TC', 'Microsoft JhengHei', 'sans-serif'],
        display: ['-apple-system', 'BlinkMacSystemFont', 'SF Pro Display', 'Inter', 'system-ui', 'PingFang TC', 'Noto Sans TC', 'sans-serif'],
        mono: ['ui-monospace', 'SFMono-Regular', 'SF Mono', 'Menlo', 'Consolas', 'monospace'],
      },
      borderRadius: { xl: '0.75rem', '2xl': '1rem' },
      spacing: { 13: '3.25rem' },
      letterSpacing: { tightest: '-0.022em' },
      boxShadow: { pop: '0 12px 32px rgba(0,0,0,0.18)' },
    },
  },
};
