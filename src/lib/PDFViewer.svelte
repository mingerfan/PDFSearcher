<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  const { filePath, initialPage = 1, onClose } = $props<{ filePath: string; initialPage?: number; onClose: () => void}>();

  let pdfData = $state<string>("");
  let currentPage = $state(initialPage);
  let totalPages = $state(1);
  let loading = $state(true);
  let error = $state("");
  let pdfCanvas = $state<HTMLCanvasElement>();
  let pdfDoc: any = null;
  let scale = $state(1.2);  onMount(async () => {
    try {
      // 动态导入 PDF.js
      const pdfjsLib = await import("pdfjs-dist");
      
      // 设置 worker - 使用本地静态文件
      pdfjsLib.GlobalWorkerOptions.workerSrc = "/js/pdf.worker.min.js";

      console.log("正在加载PDF文件:", filePath);

      // 获取PDF的base64数据
      const base64Data = await invoke<string>("get_pdf_base64", {
        filePath: filePath,
      });

      console.log("PDF base64数据长度:", base64Data.length);

      // 转换为Uint8Array
      const binaryString = atob(base64Data);
      const bytes = new Uint8Array(binaryString.length);
      for (let i = 0; i < binaryString.length; i++) {
        bytes[i] = binaryString.charCodeAt(i);
      }

      console.log("PDF二进制数据长度:", bytes.length);

      // 加载PDF文档
      const loadingTask = pdfjsLib.getDocument({ data: bytes });
      pdfDoc = await loadingTask.promise;
      totalPages = pdfDoc.numPages;
      
      console.log("PDF加载成功，总页数:", totalPages);
      
      // 渲染初始页面
      await renderPage(currentPage);
      loading = false;
    } catch (e) {
      console.error("Failed to load PDF:", e);
      error = `无法加载PDF文件: ${e}`;
      loading = false;
    }
  });

  async function renderPage(pageNum: number) {
    if (!pdfDoc || !pdfCanvas) return;

    try {
      const page = await pdfDoc.getPage(pageNum);
      const viewport = page.getViewport({ scale });
      
      pdfCanvas.height = viewport.height;
      pdfCanvas.width = viewport.width;

      const context = pdfCanvas.getContext("2d");
      const renderContext = {
        canvasContext: context,
        viewport: viewport,
      };

      await page.render(renderContext).promise;
    } catch (e) {
      console.error("Failed to render page:", e);
      error = "无法渲染PDF页面";
    }
  }

  async function goToPage(pageNum: number) {
    if (pageNum >= 1 && pageNum <= totalPages && pageNum !== currentPage) {
      currentPage = pageNum;
      await renderPage(currentPage);
    }
  }

  async function nextPage() {
    await goToPage(currentPage + 1);
  }

  async function prevPage() {
    await goToPage(currentPage - 1);
  }

  async function zoomIn() {
    scale = Math.min(scale * 1.2, 3.0);
    await renderPage(currentPage);
  }

  async function zoomOut() {
    scale = Math.max(scale / 1.2, 0.5);
    await renderPage(currentPage);
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Escape") {
      onClose();
    } else if (event.key === "ArrowLeft") {
      prevPage();
    } else if (event.key === "ArrowRight") {
      nextPage();
    } else if (event.key === "+" || event.key === "=") {
      zoomIn();
    } else if (event.key === "-") {
      zoomOut();
    }
  }
</script>

<svelte:window on:keydown={handleKeydown} />

<div class="pdf-viewer-overlay" onclick={onClose} role="button" tabindex="0" onkeydown={(e) => e.key === 'Enter' && onClose()}>
  <div class="pdf-viewer" onclick={(e) => e.stopPropagation()} role="dialog" tabindex="-1">
    <div class="pdf-toolbar">
      <div class="pdf-toolbar-left">
        <button onclick={onClose} class="close-btn">✕</button>
        <span class="file-name">{filePath.split("\\").pop()}</span>
      </div>
      
      <div class="pdf-toolbar-center">
        <button onclick={prevPage} disabled={currentPage <= 1}>
          ◀
        </button>
        <span class="page-info">
          {currentPage} / {totalPages}
        </span>
        <button onclick={nextPage} disabled={currentPage >= totalPages}>
          ▶
        </button>
      </div>

      <div class="pdf-toolbar-right">
        <button onclick={zoomOut}>🔍-</button>
        <span class="zoom-info">{Math.round(scale * 100)}%</span>
        <button onclick={zoomIn}>🔍+</button>
      </div>
    </div>

    <div class="pdf-content">
      {#if loading}
        <div class="loading">加载中...</div>
      {:else if error}
        <div class="error">{error}</div>
      {:else}
        <canvas bind:this={pdfCanvas} class="pdf-canvas"></canvas>
      {/if}
    </div>
  </div>
</div>

<style>
  .pdf-viewer-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background-color: rgba(0, 0, 0, 0.8);
    display: flex;
    justify-content: center;
    align-items: center;
    z-index: 1000;
  }

  .pdf-viewer {
    background: white;
    border-radius: 8px;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.3);
    width: 90vw;
    height: 90vh;
    display: flex;
    flex-direction: column;
  }

  .pdf-toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 1rem;
    background: #f8f9fa;
    border-bottom: 1px solid #dee2e6;
    border-radius: 8px 8px 0 0;
  }

  .pdf-toolbar-left {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .pdf-toolbar-center {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .pdf-toolbar-right {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .close-btn {
    background: #dc3545;
    color: white;
    border: none;
    border-radius: 4px;
    padding: 0.5rem;
    cursor: pointer;
    font-weight: bold;
  }

  .close-btn:hover {
    background: #c82333;
  }

  .file-name {
    font-weight: 500;
    color: #495057;
    max-width: 300px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .page-info, .zoom-info {
    font-weight: 500;
    color: #495057;
    min-width: 80px;
    text-align: center;
  }

  button {
    background: #007bff;
    color: white;
    border: none;
    border-radius: 4px;
    padding: 0.5rem 1rem;
    cursor: pointer;
    font-size: 1rem;
  }

  button:hover:not(:disabled) {
    background: #0056b3;
  }

  button:disabled {
    background: #6c757d;
    cursor: not-allowed;
  }

  .pdf-content {
    flex: 1;
    overflow: auto;
    padding: 1rem;
    display: flex;
    justify-content: center;
    align-items: flex-start;
  }

  .pdf-canvas {
    max-width: 100%;
    max-height: 100%;
    box-shadow: 0 2px 10px rgba(0, 0, 0, 0.1);
  }

  .loading, .error {
    display: flex;
    justify-content: center;
    align-items: center;
    height: 200px;
    font-size: 1.2rem;
    color: #6c757d;
  }

  .error {
    color: #dc3545;
  }
</style>
