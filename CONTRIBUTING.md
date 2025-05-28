# 贡献指南

感谢您对 PDF搜索工具 项目的关注！我们欢迎任何形式的贡献。

## 如何贡献

### 报告问题

如果您发现了 bug 或有功能建议，请：

1. 检查 [Issues](https://github.com/your-username/pdfsearcher-tauri/issues) 页面确认问题尚未被报告
2. 使用适当的 issue 模板创建新的 issue
3. 提供尽可能详细的信息

### 代码贡献

1. **Fork 项目**
   ```bash
   git clone https://github.com/your-username/pdfsearcher-tauri.git
   cd pdfsearcher-tauri
   ```

2. **创建功能分支**
   ```bash
   git checkout -b feature/your-feature-name
   ```

3. **安装依赖**
   ```bash
   npm install
   ```

4. **开发和测试**
   ```bash
   # 开发模式
   npm run tauri dev
   
   # 代码检查
   npm run check
   
   # 构建测试
   npm run tauri build
   ```

5. **提交更改**
   ```bash
   git add .
   git commit -m "feat: 添加新功能描述"
   ```

6. **推送分支**
   ```bash
   git push origin feature/your-feature-name
   ```

7. **创建 Pull Request**

## 开发环境设置

### 前提条件

- Node.js 18+
- Rust 1.70+
- 系统依赖（根据平台）:
  - **Windows**: Microsoft C++ Build Tools
  - **macOS**: Xcode Command Line Tools
  - **Linux**: `libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf`

### 代码风格

- 前端代码遵循 TypeScript 和 Svelte 最佳实践
- Rust 代码使用 `cargo fmt` 格式化
- 提交信息遵循 [Conventional Commits](https://www.conventionalcommits.org/) 规范

### 提交信息格式

```
type(scope): 简短描述

详细描述（可选）

Fixes #issue-number
```

**类型 (type):**
- `feat`: 新功能
- `fix`: 修复 bug
- `docs`: 文档更新
- `style`: 代码格式（不影响功能）
- `refactor`: 重构代码
- `test`: 添加测试
- `chore`: 构建过程或工具更改

## 测试

在提交之前，请确保：

1. 所有现有测试通过
2. 新功能包含适当的测试
3. 代码通过 lint 检查
4. 在目标平台上测试构建

## 发布流程

1. 更新版本号（`package.json` 和 `src-tauri/Cargo.toml`）
2. 更新 CHANGELOG.md（如果有）
3. 创建并推送版本标签
4. GitHub Actions 将自动构建和发布

## 获得帮助

如果您需要帮助，可以：

1. 查看 [README.md](README.md) 文档
2. 在 Issues 中搜索相关问题
3. 创建新的 issue 询问

再次感谢您的贡献！
