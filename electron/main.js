const { app, BrowserWindow } = require('electron');
const path = require('path');
const fs = require('fs');

let mainWindow = null;
let server = null;

const isDev = !app.isPackaged;

async function createWindow() {
  // 在打包后的应用中，数据目录使用 userData
  const dataDir = isDev
    ? path.join(__dirname, '../backend/data')
    : path.join(app.getPath('userData'), 'data');

  // 设置环境变量供后端使用
  process.env.DATA_DIR = dataDir;
  process.env.PORT = '0'; // 让 OS 分配空闲端口

  // 启动 Express 服务器
  const { startServer } = require('../backend/src/index');
  server = await startServer();

  const port = server.address().port;

  // 创建浏览器窗口
  mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    minWidth: 800,
    minHeight: 600,
    title: 'MangaViewer',
    titleBarStyle: 'hiddenInset', // macOS 风格标题栏
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
    },
  });

  // 加载本地服务器
  mainWindow.loadURL(`http://localhost:${port}`);

  // 开发模式下打开 DevTools
  if (isDev) {
    mainWindow.webContents.openDevTools();
  }

  mainWindow.on('closed', () => {
    mainWindow = null;
  });
}

app.whenReady().then(createWindow);

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

app.on('activate', () => {
  if (mainWindow === null) {
    createWindow();
  }
});

app.on('before-quit', () => {
  if (server) {
    server.close();
  }
});
